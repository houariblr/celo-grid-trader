import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/**
 * Format a number as USD currency with 4 decimal places.
 * Works for both raw float prices and wei-converted values.
 */
export function formatCurrency(value: number): string {
  return new Intl.NumberFormat('en-US', {
    style:                 'currency',
    currency:              'USD',
    minimumFractionDigits: 4,
    maximumFractionDigits: 4,
  }).format(value);
}

/**
 * Shorten a 0x address for display: 0x1234...5678
 */
export function shortAddr(address: string): string {
  if (!address || address.length < 10) return address;
  return `${address.slice(0, 6)}...${address.slice(-4)}`;
}

/**
 * Format a Unix timestamp (seconds) or ISO string to HH:MM:SS
 */
export function formatTime(ts: string | number): string {
  const d = typeof ts === 'number' ? new Date(ts * 1000) : new Date(ts);
  return d.toLocaleTimeString([], { hour12: false });
}

/**
 * Clamp a value between min and max.
 */
export function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}
