import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "../../lib/utils";

/**
 * 第 64 / 88 章：统一卡片。
 *
 * 与 `Button` 同样的思路：`variant` 直接映射到项目既有的 `suna-*-card` 样式类，
 * 因此 `<div className="suna-mcp-card">` → `<Card variant="mcp">` 渲染出的
 * 标签与 class 完全一致（`as` 用于保留 `article` 这类语义标签），**视觉零变化**。
 */

const cardVariants = cva("", {
  variants: {
    variant: {
      none: "",
      settings: "suna-settings-card",
      management: "suna-management-card",
      mcp: "suna-mcp-card",
      diagnostics: "suna-diagnostics-card",
      stat: "suna-stat-card",
      registry: "suna-registry-card",
      plan: "suna-plan-card",
      account: "suna-account-profile-card",
      library: "suna-knowledge-library-card",
      welcome: "suna-welcome-card",
      skill: "suna-skill-card",
      settingsSkill: "suna-settings-skill-card",
      researchSummary: "suna-research-summary-card",
      themePreview: "suna-theme-preview-card",
    },
  },
  defaultVariants: { variant: "none" },
});

type CardElement = "div" | "article" | "section" | "aside";

export interface CardProps
  extends React.HTMLAttributes<HTMLElement>,
    VariantProps<typeof cardVariants> {
  /** 渲染的语义标签；默认 `div`。 */
  as?: CardElement;
}

const Card = React.forwardRef<HTMLElement, CardProps>(
  ({ className, variant, as: Tag = "div", ...props }, ref) => (
    <Tag ref={ref as never} className={cn(cardVariants({ variant }), className)} {...props} />
  ),
);
Card.displayName = "Card";

const CardHeader = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div ref={ref} className={cn("flex flex-col gap-1 p-4", className)} {...props} />
  ),
);
CardHeader.displayName = "CardHeader";

const CardTitle = React.forwardRef<HTMLHeadingElement, React.HTMLAttributes<HTMLHeadingElement>>(
  ({ className, ...props }, ref) => (
    <h3 ref={ref} className={cn("text-sm font-medium text-foreground", className)} {...props} />
  ),
);
CardTitle.displayName = "CardTitle";

const CardDescription = React.forwardRef<HTMLParagraphElement, React.HTMLAttributes<HTMLParagraphElement>>(
  ({ className, ...props }, ref) => (
    <p ref={ref} className={cn("text-xs text-muted-foreground", className)} {...props} />
  ),
);
CardDescription.displayName = "CardDescription";

const CardContent = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div ref={ref} className={cn("p-4 pt-0", className)} {...props} />
  ),
);
CardContent.displayName = "CardContent";

const CardFooter = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div ref={ref} className={cn("flex items-center gap-2 p-4 pt-0", className)} {...props} />
  ),
);
CardFooter.displayName = "CardFooter";

export { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter, cardVariants };
