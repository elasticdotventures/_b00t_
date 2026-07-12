# TypeScript Canonical Patterns — assimilated from AllThingsSmitty/typescript-tips-everyone-should-know
# 🤓 b00t learn typescript  →  loads all 15 patterns
# 🤓 b00t grok ask "how to narrow types in typescript" -t typescript  → semantic search
# 🤓 CC0-1.0 license — public domain, safe to use everywhere
# Source: https://github.com/AllThingsSmitty/typescript-tips-everyone-should-know

## 1. Prefer `unknown` over `any`

`unknown` forces type narrowing before use. `any` disables the type system.

```typescript
function parse(data: unknown) {
  if (typeof data === "string") return data.toUpperCase(); // OK
  // data.toUpperCase(); // ERROR without check
}
```

## 2. Let inference do the work

Don't annotate types the compiler already knows.

```typescript
const name = "Ada";           // inferred: "Ada"
const nums = [1, 2, 3];       // inferred: number[]
// Better than: const name: string = "Ada"  — widens unnecessarily
```

## 3. Prefer `satisfies` over `as`

`satisfies` validates shape while preserving literal types. `as` silently widens.

```typescript
const routes = { home: "/", about: "/about" } satisfies Record<string, string>;
// routes.home is "/" (literal), not string
```

## 4. Derive types from values (single source of truth)

Don't duplicate types from runtime values.

```typescript
const roles = ["admin", "user", "guest"] as const;
type Role = (typeof roles)[number];  // "admin" | "user" | "guest"
```

## 5. Make invalid states impossible (discriminated unions)

Prevent impossible combinations at the type level.

```typescript
type State =
  | { status: "loading" }
  | { status: "success"; data: User }
  | { status: "error"; error: Error };
// Cannot have data while loading. Cannot have error while success.
```

## 6. Exhaustive checks with `never`

Compiler catches missing cases when you add new states.

```typescript
default: {
  const _exhaustive: never = state; // Error if new state unhandled
  return _exhaustive;
}
```

## 7. `as const` for constants

Preserves literal types instead of widening.

```typescript
const theme = { mode: "dark" } as const;
// typeof theme.mode = "dark" (not string)
```

## 8. Type predicates for reusable narrowing

Connect runtime checks to compile-time type narrowing.

```typescript
function isUser(v: unknown): v is User {
  return typeof v === "object" && v !== null && "id" in v;
}
if (isUser(data)) data.id; // narrowed to User
```

## 9. Build types from existing types (utility types)

Transform, don't duplicate.

```typescript
type Preview = Pick<User, "id" | "name">;
type NoId = Omit<User, "id">;
type PartialUser = Partial<User>;
```
Key utilities: `Pick`, `Omit`, `Partial`, `Required`, `ReturnType`, `Parameters`, indexed access `T["key"]`.

## 10. Validate external data at runtime (zod)

TypeScript types disappear at runtime. Validate API boundaries.

```typescript
const UserSchema = z.object({ id: z.string(), name: z.string() });
type User = z.infer<typeof UserSchema>;
```

## 11. Avoid `enum` — use `as const` unions

Literal unions are simpler to refactor, serialize, and type-narrow.

```typescript
const roles = ["admin", "user"] as const;  // prefer this
// enum Role { Admin, User }              // over this
```

## 12. Prefer inferable generics

Let TypeScript infer generic parameters from arguments.

```typescript
function first<T>(arr: T[]): T { return arr[0]; }
first([1, 2, 3]); // T inferred as number, no explicit annotation
```

## 13. Enable strict compiler options

The real payoff of TypeScript:

```json
{
  "strict": true,
  "useUnknownInCatchVariables": true,
  "noUncheckedIndexedAccess": true,
  "exactOptionalPropertyTypes": true
}
```

## 14. Template literal types

Powerful for routes, events, CSS, and query keys.

```typescript
type Route = `/api/${string}`;
type EventName = `on${Capitalize<string>}`;
```

## 15. Type-safe ≠ runtime safe

TypeScript improves correctness but doesn't validate at runtime. Always validate external boundaries (APIs, forms, env vars, user input).
