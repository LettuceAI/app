export const typography = {
    display: {
        size: 'text-2xl',        // 24px
        weight: 'font-extrabold',
        lineHeight: 'leading-tight',
        tracking: 'tracking-tight',
    },
    h1: {
        size: 'text-xl',         // 20px
        weight: 'font-bold',
        lineHeight: 'leading-snug',
        tracking: 'tracking-tight',
    },
    h2: {
        size: 'text-lg',         // 18px
        weight: 'font-bold',
        lineHeight: 'leading-snug',
        tracking: 'tracking-tight',
    },
    h3: {
        size: 'text-base',       // 16px
        weight: 'font-semibold',
        lineHeight: 'leading-normal',
    },
    heading: {
        size: 'text-xl',        
        weight: 'font-semibold',
        lineHeight: 'leading-relaxed',
    },
    body: {
        size: 'text-sm',         // 14px
        weight: 'font-normal',
        lineHeight: 'leading-relaxed',
    },
    bodySmall: {
        size: 'text-[12px]',         // 12px
        weight: 'font-normal',
        lineHeight: 'leading-normal',
    },
    label: {
        size: 'text-[12px]',         // 12px
        weight: 'font-medium',
        lineHeight: 'leading-none',
        tracking: 'tracking-wide', // 0.025em
    },
    caption: {
        size: 'text-[11px]',     // 11px
        weight: 'font-medium',
        lineHeight: 'leading-none',
    },
    overline: {
        size: 'text-[10px]',    // 10px
        weight: 'font-bold',
        lineHeight: 'leading-none',
        tracking: 'tracking-wider', // 0.05em
        transform: 'uppercase',
    },
} as const;

export const colors = {
    surface: {
        base: 'bg-surface',
        elevated: 'bg-surface-el',
        nav: 'bg-nav',
        overlay: 'bg-black/60',
    },
    border: {
        subtle: 'border-fg/15',
        default: 'border-fg/20',
        strong: 'border-fg/30',
        interactive: 'border-fg/40',
    },
    text: {
        primary: 'text-fg',
        secondary: 'text-fg/80',
        tertiary: 'text-fg/50',
        disabled: 'text-fg/30',
    },
    accent: {
        emerald: {
            subtle: 'bg-accent/10 border-accent/30 text-accent/80',
            default: 'bg-accent/20 border-accent/40 text-accent/90',
            strong: 'bg-accent/30 border-accent/60 text-accent',
        },
        amber: {
            subtle: 'bg-warning/10 border-warning/30 text-warning/80',
            default: 'bg-warning/15 border-warning/40 text-warning/90',
        },
        red: {
            subtle: 'bg-danger/10 border-danger/30 text-danger/80',
            default: 'bg-danger/15 border-danger/40 text-danger/90',
        },
        blue: {
            subtle: 'bg-info/10 border-info/30 text-info/80',
            default: 'bg-info/15 border-info/40 text-info/90',
        },
        purple: {
            subtle: 'bg-secondary/10 border-secondary/30 text-secondary/80',
            default: 'bg-secondary/15 border-secondary/40 text-secondary/90',
        },
    },
    glass: {
        subtle: 'bg-fg/5 backdrop-blur-lg border border-fg/10',
        default: 'bg-fg/10 backdrop-blur-xl border border-fg/15',
        strong: 'bg-surface-el/80 backdrop-blur-xl border-b border-fg/10',
    },
    effects: {
        glow: 'shadow-[0_0_20px_-5px_rgba(52,211,153,0.3)]',
        gradient: {
            brand: 'bg-gradient-to-r from-accent/20 via-blue-500/20 to-accent/20',
            surface: 'bg-gradient-to-b from-surface to-black',
            text: 'bg-gradient-to-r from-fg via-fg to-fg/70 bg-clip-text text-transparent',
        }
    }
} as const;

export const spacing = {
    section: 'space-y-6',
    group: 'space-y-4',
    item: 'space-y-3',
    field: 'space-y-2',
    tight: 'space-y-1.5',
    inline: 'gap-3',
    inlineSmall: 'gap-2',
} as const;

export const radius = {
    sm: 'rounded-sm',
    md: 'rounded-md',
    lg: 'rounded-lg',
    full: 'rounded-full',
} as const;

export const shadows = {
    sm: 'shadow-[0_2px_8px_rgba(0,0,0,0.2)]',
    md: 'shadow-[0_4px_16px_rgba(0,0,0,0.3)]',
    lg: 'shadow-[0_8px_24px_rgba(0,0,0,0.4)]',
    xl: 'shadow-[0_12px_32px_rgba(0,0,0,0.5)]',
    glow: 'shadow-[0_8px_24px_rgba(52,211,153,0.15)]',
} as const;

export const interactive = {
    hover: {
        scale: 'hover:scale-[1.02]',
        opacity: 'hover:opacity-90',
        brightness: 'hover:brightness-110',
    },
    active: {
        scale: 'active:scale-[0.98]',
    },
    transition: {
        fast: 'transition-all duration-150',
        default: 'transition-all duration-200',
        slow: 'transition-all duration-300',
    },
    focus: {
        ring: 'focus:outline-none focus:ring-2 focus:ring-fg/20',
    },
} as const;

export const components = {
    button: {
        primary: `${radius.md} ${interactive.transition.fast} ${interactive.active.scale} ${interactive.focus.ring}`,
        sizes: {
            sm: 'px-3 py-1.5 text-xs',
            md: 'px-4 py-2.5 text-sm',
            lg: 'px-6 py-3 text-base',
        },
    },
    input: {
        base: `${radius.md} border ${colors.border.subtle} bg-surface/20 backdrop-blur-xl ${interactive.transition.default} focus:${colors.border.interactive} focus:bg-surface/30 focus:outline-none`,
        sizes: {
            sm: 'px-3 py-2 text-xs',
            md: 'px-4 py-3 text-sm',
            lg: 'px-4 py-3.5 text-base',
        },
    },
    card: {
        base: `${radius.lg} border ${colors.border.subtle} bg-fg/5 ${interactive.transition.default}`,
        interactive: `hover:${colors.border.strong} hover:bg-fg/10 ${interactive.active.scale}`,
    },
    listItem: {
        base: `${radius.md} border ${colors.border.subtle} bg-fg/5 ${interactive.transition.default}`,
        interactive: `hover:${colors.border.strong} hover:bg-fg/10 ${interactive.active.scale}`,
    },
} as const;

export const animations = {
    fadeIn: {
        initial: { opacity: 0, y: 16 },
        animate: { opacity: 1, y: 0 },
        exit: { opacity: 0, y: -16 },
        transition: { duration: 0.2, ease: [0.25, 0.46, 0.45, 0.94] },
    },
    fadeInFast: {
        initial: { opacity: 0, y: 8 },
        animate: { opacity: 1, y: 0 },
        exit: { opacity: 0, y: -8 },
        transition: { duration: 0.15, ease: 'easeOut' },
    },
    slideUp: {
        initial: { opacity: 0, y: 32 },
        animate: { opacity: 1, y: 0 },
        exit: { opacity: 0, y: 32 },
        transition: { duration: 0.3, ease: [0.25, 0.46, 0.45, 0.94] },
    },
    scaleIn: {
        initial: { opacity: 0, scale: 0.95 },
        animate: { opacity: 1, scale: 1 },
        exit: { opacity: 0, scale: 0.95 },
        transition: { duration: 0.2 },
    },
} as const;

export function cn(...classes: (string | boolean | undefined | null)[]): string {
    return classes.filter(Boolean).join(' ');
}
