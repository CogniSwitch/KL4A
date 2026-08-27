import type { ButtonHTMLAttributes } from 'react';

type ButtonVariant = 'primary' | 'secondary' | 'danger' | 'ghost';

const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  primary: 'rounded-lg bg-accent px-3 py-1.5 text-white hover:bg-accent-hover disabled:opacity-40',
  secondary: 'rounded-lg border border-line px-3 py-1.5 text-ink hover:bg-panel-muted disabled:opacity-40',
  danger: 'rounded-lg border border-bad/30 px-3 py-1.5 text-bad hover:bg-bad-soft disabled:opacity-40',
  ghost: 'text-accent underline decoration-accent/40 underline-offset-2 hover:decoration-accent disabled:opacity-40',
};

export function Button({
  variant = 'secondary',
  className = '',
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: ButtonVariant }) {
  return (
    <button
      type="button"
      className={`text-sm font-medium transition-colors ${VARIANT_CLASSES[variant]} ${className}`}
      {...props}
    />
  );
}
