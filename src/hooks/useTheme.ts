import { useEffect } from "react";
import { applyTheme } from "../themes/themes";

export function useTheme(themeName: string, styleName?: string, animationName?: string) {
  useEffect(() => {
    applyTheme(themeName);
  }, [themeName]);

  useEffect(() => {
    const root = document.documentElement;
    // Remove all style classes
    root.classList.remove("style-default", "style-minimal", "style-glass", "style-neon");
    if (styleName && styleName !== "default") {
      root.classList.add(`style-${styleName}`);
    }
  }, [styleName]);

  useEffect(() => {
    const root = document.documentElement;
    root.classList.remove("anim-none", "anim-smooth", "anim-energetic");
    const anim = animationName || "smooth";
    root.classList.add(`anim-${anim}`);
  }, [animationName]);
}
