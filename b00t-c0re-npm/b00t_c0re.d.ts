/* tslint:disable */
/* eslint-disable */
/**
 * Core b00t framework version info
 */
export function b00t_version(): string;
/**
 * Core b00t greeting - stay aligned!
 */
export function b00t_greet(name: string): string;
/**
 * Check if a command looks like a slash command
 */
export function is_slash_command(input: string): boolean;
/**
 * Parse slash command (simplified version)
 */
export function parse_slash_command(input: string): string;
/**
 * Initialize b00t core - call this when loading
 */
export function main(): void;
