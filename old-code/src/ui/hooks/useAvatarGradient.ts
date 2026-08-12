import { useState, useEffect, useMemo, useCallback } from "react";
import {
    AVATAR_UPDATED_EVENT,
    getCachedGradient,
    peekCachedGradient,
    type AvatarGradient,
    type EntityType,
} from "../../core/storage/avatars";
import type { AvatarGradientSource } from "../../core/storage/schemas";

/**
 * Custom gradient colors configuration
 */
export interface CustomGradientColors {
    colors?: string[]; // Array of hex colors
    textColor?: string; // Custom text color hex
    textSecondary?: string; // Custom secondary text color hex
}

/**
 * Generate gradient CSS from an array of hex colors
 * Array order: [start, end] or [start, end, middle]
 * Gradient order: start -> middle (if exists) -> end
 */
function generateGradientFromColors(colors: string[]): string {
    if (!colors || colors.length === 0) return "";
    if (colors.length === 1) {
        return colors[0];
    }
    if (colors.length === 2) {
        return `linear-gradient(135deg, ${colors[0]}, ${colors[1]})`;
    }
    // 3+ colors: reorder as start, middle, end
    const orderedColors = [colors[0], colors[2], colors[1]];
    return `linear-gradient(135deg, ${orderedColors.join(", ")})`;
}

/**
 * Calculate if text should be light or dark based on background color
 */
function calculateTextColorFromGradient(colors: string[]): { textColor: string; textSecondary: string } {
    if (!colors || colors.length === 0) {
        return { textColor: "#ffffff", textSecondary: "rgba(255, 255, 255, 0.7)" };
    }

    // Average the luminance of all colors
    let totalLuminance = 0;
    for (const hex of colors) {
        const rgb = hexToRgb(hex);
        if (rgb) {
            const luminance = 0.299 * rgb.r + 0.587 * rgb.g + 0.114 * rgb.b;
            totalLuminance += luminance;
        }
    }
    const avgLuminance = totalLuminance / colors.length;

    // If bright background, use dark text
    if (avgLuminance > 140) {
        return { textColor: "#1a1a1a", textSecondary: "rgba(0, 0, 0, 0.6)" };
    }
    return { textColor: "#ffffff", textSecondary: "rgba(255, 255, 255, 0.7)" };
}

function hexToRgb(hex: string): { r: number; g: number; b: number } | null {
    const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
    return result ? {
        r: parseInt(result[1], 16),
        g: parseInt(result[2], 16),
        b: parseInt(result[3], 16)
    } : null;
}

/**
 * Hook to generate and use gradient colors from avatar images
 * Perfect for character cards, persona cards, and other avatar-based UI elements
 * 
 * @param type - Entity type (character or persona)
 * @param entityId - The entity ID
 * @param avatarPath - Optional avatar path from the entity
 * @param disabled - If true, gradient generation is disabled
 * @param customColors - Optional custom colors to override auto-detected gradient
 * @returns Object with gradient data and loading state
 */
export function useAvatarGradient(
    type: EntityType,
    entityId: string,
    avatarPath?: string,
    disabled?: boolean,
    customColors?: CustomGradientColors,
    source: AvatarGradientSource = "round",
) {
    const hasCustomColors = Boolean(customColors?.colors && customColors.colors.length > 0);
    const cachedGradient =
        entityId && !hasCustomColors ? peekCachedGradient(type, entityId, source) ?? null : null;

    const [gradient, setGradient] = useState<AvatarGradient | null>(() =>
        cachedGradient
    );
    const [isLoading, setIsLoading] = useState(() =>
        Boolean(entityId && avatarPath && !disabled && !hasCustomColors && !cachedGradient)
    );
    const [error, setError] = useState<string | null>(null);
    const [refreshTick, setRefreshTick] = useState(0);

    // Custom gradient - memoized to avoid recalculation
    const customGradient = useMemo(() => {
        if (!hasCustomColors || !customColors?.colors) return null;

        const gradientCss = generateGradientFromColors(customColors.colors);
        const autoTextColors = calculateTextColorFromGradient(customColors.colors);

        return {
            gradientCss,
            textColor: customColors.textColor || autoTextColors.textColor,
            textSecondary: customColors.textSecondary || autoTextColors.textSecondary,
        };
    }, [hasCustomColors, customColors?.colors, customColors?.textColor, customColors?.textSecondary]);

    const refreshGradient = useCallback(async (force = false) => {
        if (hasCustomColors) {
            setGradient(null);
            return null;
        }

        if (!entityId || !avatarPath || disabled) {
            setGradient(null);
            return null;
        }

        const existingGradient = peekCachedGradient(type, entityId, source) ?? null;
        if (existingGradient) {
            setGradient((current) => current ?? existingGradient);
            if (!force) {
                setIsLoading(false);
                setError(null);
                return existingGradient;
            }
        }

        setIsLoading(true);
        setError(null);

        try {
            const gradientData = await getCachedGradient(type, entityId, avatarPath, force, source);
            setGradient((current) => gradientData || current || null);
            return gradientData || null;
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : "Failed to generate gradient";
            setError(errorMessage);
            if (!existingGradient) {
                setGradient(null);
            }
            throw err;
        } finally {
            setIsLoading(false);
        }
    }, [avatarPath, disabled, entityId, hasCustomColors, source, type]);

    useEffect(() => {
        refreshGradient().catch(() => undefined);
    }, [refreshGradient, refreshTick]);

    useEffect(() => {
        const handleAvatarUpdated = (event: Event) => {
            const detail = (event as CustomEvent<{ type?: EntityType; entityId?: string }>).detail;
            if (!detail || detail.type !== type || detail.entityId !== entityId) {
                return;
            }
            setRefreshTick((value) => value + 1);
        };

        window.addEventListener(AVATAR_UPDATED_EVENT, handleAvatarUpdated as EventListener);
        return () => {
            window.removeEventListener(AVATAR_UPDATED_EVENT, handleAvatarUpdated as EventListener);
        };
    }, [entityId, type]);

    // Calculate average brightness from gradient colors
    const calculateAverageBrightness = (): number => {
        if (!gradient || !gradient.colors || gradient.colors.length === 0) {
            return 0.5; // Default mid brightness
        }

        let totalBrightness = 0;
        for (const color of gradient.colors) {
            const r = color.r / 255.0;
            const g = color.g / 255.0;
            const b = color.b / 255.0;

            const rLin = r <= 0.03928 ? r / 12.92 : Math.pow((r + 0.055) / 1.055, 2.4);
            const gLin = g <= 0.03928 ? g / 12.92 : Math.pow((g + 0.055) / 1.055, 2.4);
            const bLin = b <= 0.03928 ? b / 12.92 : Math.pow((b + 0.055) / 1.055, 2.4);

            const luminance = 0.2126 * rLin + 0.7152 * gLin + 0.0722 * bLin;
            totalBrightness += luminance;
        }

        return totalBrightness / gradient.colors.length;
    };

    const averageBrightness = calculateAverageBrightness();

    // Return custom gradient if available, otherwise auto-detected
    if (hasCustomColors && customGradient) {
        return {
            gradient: null,
            isLoading: false,
            error: null,
            hasGradient: true,
            gradientCss: customGradient.gradientCss,
            colors: [],
            dominantHue: 0,
            textColor: customGradient.textColor,
            textSecondary: customGradient.textSecondary,
            averageBrightness: 0.5,
            isCustom: true,
            refreshGradient,
        };
    }

    return {
        gradient,
        isLoading,
        error,
        hasGradient: !!gradient,
        gradientCss: gradient?.gradient_css || "",
        colors: gradient?.colors || [],
        dominantHue: gradient?.dominant_hue || 0,
        textColor: gradient?.text_color || "#ffffff",
        textSecondary: gradient?.text_secondary || "rgba(255, 255, 255, 0.7)",
        averageBrightness,
        isCustom: false,
        refreshGradient,
    };
}

/**
 * Hook for generating multiple gradients at once
 * Useful for lists and grids of characters/personas
 * 
 * @param entities - Array of entities with type, id, avatar path, and optional disabled flag
 * @returns Map of entityId to gradient data
 */
export function useMultipleAvatarGradients(
    entities: Array<{
        type: EntityType;
        id: string;
        avatarPath?: string;
        disableGradient?: boolean;
        source?: AvatarGradientSource;
    }>
) {
    const [gradients, setGradients] = useState<Map<string, AvatarGradient>>(new Map());
    const [isLoading, setIsLoading] = useState(false);
    const [refreshTick, setRefreshTick] = useState(0);

    const entitiesKey = JSON.stringify(
        entities.map(e => ({
            type: e.type,
            id: e.id,
            path: e.avatarPath,
            disabled: e.disableGradient,
            source: e.source ?? "round",
        }))
    );

    useEffect(() => {
        if (entities.length === 0) {
            setGradients(new Map());
            return;
        }

        const generateGradients = async () => {
            setIsLoading(true);
            const newGradients = new Map<string, AvatarGradient>();

            try {
                const gradientPromises = entities
                    .filter(entity => entity.id && entity.avatarPath && !entity.disableGradient)
                    .map(async (entity) => {
                        try {
                            const gradient = await getCachedGradient(
                                entity.type,
                                entity.id,
                                entity.avatarPath!,
                                false,
                                entity.source ?? "round",
                            );
                            return { id: entity.id, gradient };
                        } catch {
                            return { id: entity.id, gradient: null };
                        }
                    });

                const results = await Promise.all(gradientPromises);

                for (const { id, gradient } of results) {
                    if (gradient) {
                        newGradients.set(id, gradient);
                    }
                }
            } catch (err) {
                console.error("[useMultipleAvatarGradients] Failed to generate gradients:", err);
            } finally {
                setGradients(newGradients);
                setIsLoading(false);
            }
        };

        generateGradients();
    }, [entitiesKey, refreshTick]);

    useEffect(() => {
        const relevantKeys = new Set(entities.map((entity) => `${entity.type}:${entity.id}`));
        const handleAvatarUpdated = (event: Event) => {
            const detail = (event as CustomEvent<{ type?: EntityType; entityId?: string }>).detail;
            if (!detail?.type || !detail.entityId) {
                return;
            }
            if (!relevantKeys.has(`${detail.type}:${detail.entityId}`)) {
                return;
            }
            setRefreshTick((value) => value + 1);
        };

        window.addEventListener(AVATAR_UPDATED_EVENT, handleAvatarUpdated as EventListener);
        return () => {
            window.removeEventListener(AVATAR_UPDATED_EVENT, handleAvatarUpdated as EventListener);
        };
    }, [entitiesKey, entities]);

    const getGradient = (entityId: string): AvatarGradient | undefined => {
        return gradients.get(entityId);
    };

    const getGradientCss = (entityId: string): string => {
        return gradients.get(entityId)?.gradient_css || "";
    };

    const getTextColor = (entityId: string): string => {
        return gradients.get(entityId)?.text_color || "#ffffff";
    };

    const getTextSecondary = (entityId: string): string => {
        return gradients.get(entityId)?.text_secondary || "rgba(255, 255, 255, 0.7)";
    };

    return {
        gradients,
        isLoading,
        getGradient,
        getGradientCss,
        getTextColor,
        getTextSecondary,
        hasGradient: (entityId: string) => gradients.has(entityId),
    };
}
