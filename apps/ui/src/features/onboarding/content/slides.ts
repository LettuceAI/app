export const learnSlides = ["welcome", "car", "provider", "key", "engine", "cost"] as const;
export type LearnSlideId = (typeof learnSlides)[number];

export const guidedRoutes = ["gemini", "openrouter"] as const;
export type GuidedRoute = (typeof guidedRoutes)[number];
