import type { ParseKeys } from "i18next";

type CopyKey = ParseKeys<"onboarding">;

export interface GuideSlide {
  shot: string;
  caption: CopyKey;
}

export interface GuideStepContent {
  title: CopyKey;
  body: CopyKey;
  open: CopyKey | null;
  slides: readonly GuideSlide[];
}

export type GuideName = "geminiDesktop" | "geminiMobile" | "openrouterDesktop" | "openrouterMobile";

export const guideContent: Readonly<Record<GuideName, readonly GuideStepContent[]>> = {
  geminiDesktop: [
    {
      title: "guides.geminiDesktop.open.title",
      body: "guides.geminiDesktop.open.body",
      open: "guides.geminiDesktop.open.open",
      slides: [
        { shot: "google_gemini_setup_desktop_1", caption: "guides.geminiDesktop.open.slide1" },
      ],
    },
    {
      title: "guides.geminiDesktop.find.title",
      body: "guides.geminiDesktop.find.body",
      open: null,
      slides: [
        { shot: "google_gemini_setup_desktop_2", caption: "guides.geminiDesktop.find.slide1" },
      ],
    },
    {
      title: "guides.geminiDesktop.create.title",
      body: "guides.geminiDesktop.create.body",
      open: null,
      slides: [
        { shot: "google_gemini_setup_desktop_3", caption: "guides.geminiDesktop.create.slide1" },
        { shot: "google_gemini_setup_desktop_3-1", caption: "guides.geminiDesktop.create.slide2" },
        { shot: "google_gemini_setup_desktop_3-2", caption: "guides.geminiDesktop.create.slide3" },
        { shot: "google_gemini_setup_desktop_3-3", caption: "guides.geminiDesktop.create.slide4" },
      ],
    },
    {
      title: "guides.geminiDesktop.copy.title",
      body: "guides.geminiDesktop.copy.body",
      open: null,
      slides: [
        { shot: "google_gemini_setup_desktop_4", caption: "guides.geminiDesktop.copy.slide1" },
      ],
    },
  ],
  geminiMobile: [
    {
      title: "guides.geminiMobile.open.title",
      body: "guides.geminiMobile.open.body",
      open: "guides.geminiMobile.open.open",
      slides: [
        { shot: "google_gemini_setup_mobile_1", caption: "guides.geminiMobile.open.slide1" },
        { shot: "google_gemini_setup_mobile_1-2", caption: "guides.geminiMobile.open.slide2" },
      ],
    },
    {
      title: "guides.geminiMobile.find.title",
      body: "guides.geminiMobile.find.body",
      open: null,
      slides: [
        { shot: "google_gemini_setup_mobile_2", caption: "guides.geminiMobile.find.slide1" },
        { shot: "google_gemini_setup_mobile_2-2", caption: "guides.geminiMobile.find.slide2" },
      ],
    },
    {
      title: "guides.geminiMobile.start.title",
      body: "guides.geminiMobile.start.body",
      open: null,
      slides: [
        { shot: "google_gemini_setup_mobile_3", caption: "guides.geminiMobile.start.slide1" },
      ],
    },
    {
      title: "guides.geminiMobile.project.title",
      body: "guides.geminiMobile.project.body",
      open: null,
      slides: [
        { shot: "google_gemini_setup_mobile_4", caption: "guides.geminiMobile.project.slide1" },
        { shot: "google_gemini_setup_mobile_4-2", caption: "guides.geminiMobile.project.slide2" },
      ],
    },
    {
      title: "guides.geminiMobile.create.title",
      body: "guides.geminiMobile.create.body",
      open: null,
      slides: [
        { shot: "google_gemini_setup_mobile_5", caption: "guides.geminiMobile.create.slide1" },
        { shot: "google_gemini_setup_mobile_5-1", caption: "guides.geminiMobile.create.slide2" },
      ],
    },
  ],
  openrouterDesktop: [
    {
      title: "guides.openrouterDesktop.account.title",
      body: "guides.openrouterDesktop.account.body",
      open: "guides.openrouterDesktop.account.open",
      slides: [
        { shot: "openrouter_setup_desktop_1", caption: "guides.openrouterDesktop.account.slide1" },
      ],
    },
    {
      title: "guides.openrouterDesktop.key.title",
      body: "guides.openrouterDesktop.key.body",
      open: null,
      slides: [
        { shot: "openrouter_setup_desktop_2", caption: "guides.openrouterDesktop.key.slide1" },
        { shot: "openrouter_setup_desktop_2-1", caption: "guides.openrouterDesktop.key.slide2" },
      ],
    },
    {
      title: "guides.openrouterDesktop.payment.title",
      body: "guides.openrouterDesktop.payment.body",
      open: null,
      slides: [
        { shot: "openrouter_setup_desktop_3", caption: "guides.openrouterDesktop.payment.slide1" },
        { shot: "openrouter_setup_desktop_3-1", caption: "guides.openrouterDesktop.payment.slide2" },
      ],
    },
    {
      title: "guides.openrouterDesktop.credits.title",
      body: "guides.openrouterDesktop.credits.body",
      open: null,
      slides: [
        { shot: "openrouter_setup_desktop_4", caption: "guides.openrouterDesktop.credits.slide1" },
        { shot: "openrouter_setup_desktop_4-1", caption: "guides.openrouterDesktop.credits.slide2" },
      ],
    },
  ],
  openrouterMobile: [
    {
      title: "guides.openrouterMobile.account.title",
      body: "guides.openrouterMobile.account.body",
      open: "guides.openrouterMobile.account.open",
      slides: [
        { shot: "openrouter_setup_mobile_1", caption: "guides.openrouterMobile.account.slide1" },
      ],
    },
    {
      title: "guides.openrouterMobile.key.title",
      body: "guides.openrouterMobile.key.body",
      open: null,
      slides: [
        { shot: "openrouter_setup_mobile_2", caption: "guides.openrouterMobile.key.slide1" },
        { shot: "openrouter_setup_mobile_2-1", caption: "guides.openrouterMobile.key.slide2" },
      ],
    },
    {
      title: "guides.openrouterMobile.payment.title",
      body: "guides.openrouterMobile.payment.body",
      open: null,
      slides: [
        { shot: "openrouter_setup_mobile_3", caption: "guides.openrouterMobile.payment.slide1" },
        { shot: "openrouter_setup_mobile_3-1", caption: "guides.openrouterMobile.payment.slide2" },
      ],
    },
    {
      title: "guides.openrouterMobile.credits.title",
      body: "guides.openrouterMobile.credits.body",
      open: null,
      slides: [
        { shot: "openrouter_setup_mobile_4", caption: "guides.openrouterMobile.credits.slide1" },
        { shot: "openrouter_setup_mobile_4-1", caption: "guides.openrouterMobile.credits.slide2" },
      ],
    },
  ],
};
