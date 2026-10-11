import * as React from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "../../lib/utils";

/**
 * 第 63 / 88 章：统一按钮。
 *
 * 变体直接映射到项目既有的 `suna-*` 按钮样式类，因此把原来的
 * `<button className="suna-ghost-button">` 换成 `<Button variant="ghost">`
 * 渲染出的 class 完全一致，**视觉零变化**。
 *
 * `size="icon"` 对应方形图标按钮（`suna-icon-button`），通常与 `variant="none"` 搭配。
 */

const buttonVariants = cva("", {
  variants: {
    variant: {
      none: "",
      primary: "suna-primary-button",
      secondary: "suna-secondary-button",
      ghost: "suna-ghost-button",
      danger: "suna-danger-button",
      /** 侧栏/卡片里的软按钮（`suna-button is-soft`）。 */
      soft: "suna-button is-soft",
      /** 无修饰的通用按钮（`suna-button`）。 */
      plain: "suna-button",
      /** 设置页的主/次操作按钮。 */
      action: "suna-action-primary",
      actionSecondary: "suna-action-secondary",
    },
    size: {
      default: "",
      icon: "suna-icon-button",
    },
  },
  defaultVariants: { variant: "none", size: "default" },
});

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean;
}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, asChild = false, ...props }, ref) => {
    const Comp = asChild ? Slot : "button";
    return <Comp ref={ref} className={cn(buttonVariants({ variant, size }), className)} {...props} />;
  },
);
Button.displayName = "Button";

export { Button, buttonVariants };
