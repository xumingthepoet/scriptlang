# ScriptLang Syntax Manual (V1)

This manual defines the concrete XML authoring syntax for ScriptLang V1.

## 1. File and Root

- File extension: `.script.xml`
- Exactly one script per file.
- Root element must be `<script>`.

Example:

```xml
<script name="main.script.xml">
  <vars>
    <var name="hp" type="number" value="10"/>
  </vars>
  <step>
    <text value="HP is ${hp}"/>
  </step>
</script>
```

## 2. Top-Level Structure

Allowed direct children of `<script>`:

1. `<vars>` (optional, at most one)
2. `<step>` (optional, at most one; executable body)

If `<step>` is missing, script executes as an empty body.

## 3. Variable Declarations (`<vars>`)

Inside `<vars>`, only `<var>` is allowed.

`<var>` attributes:

- `name` (required): variable name, unique within this script.
- `type` (required): ScriptLang type expression.
- `value` (optional): TS expression for initial value.

Rules:

- All variables used by script logic must be declared here.
- Duplicate `name` is a compile error.
- `undefined` is not allowed as value.

### 3.1 Type Syntax

Supported type expressions:

- Primitive: `number`, `string`, `boolean`, `null`
- Array: `T[]`
- Record: `Record<string, T>`
- Map: `Map<string, T>`

Examples:

- `number`
- `string[]`
- `Record<string, boolean>`
- `Map<string, number[]>`

## 4. Executable Nodes (`<step>`)

Supported executable nodes:

1. `<text>`
2. `<code>`
3. `<if>` / `<else>`
4. `<while>`
5. `<choice>` / `<option>`
6. `<call>`
7. `<return>`

Removed in V1 (compile-time error):

- `<set>`
- `<push>`
- `<remove>`

## 5. `<text>`

Forms:

```xml
<text value="HP=${hp}"/>
```

or

```xml
<text>
  HP=${hp}
</text>
```

Supports interpolation with `${expr}`. Expression is evaluated using runtime expression engine.

## 6. `<code>`

Executes TypeScript-like statement block in sandboxed VM.

Forms:

```xml
<code>hp = hp + 1;</code>
```

or

```xml
<code value="hp = hp + 1;"/>
```

Rules:

- You can read/write declared vars and visible group-scope vars.
- Assignment to `undefined` is rejected.
- Runtime enforces declared type on script variables.
- Use host-registered functions for external logic (including random).

## 7. `<if>` / `<else>`

Syntax:

```xml
<if when="hp > 0">
  <text value="alive"/>
  <else>
    <text value="dead"/>
  </else>
</if>
```

Rules:

- `when` is required and must evaluate to `boolean`.
- `<else>` is optional.
- Both branches are compiled into implicit child groups.

## 8. `<while>`

Syntax:

```xml
<while when="hp > 0">
  <code>hp = hp - 1;</code>
</while>
```

Rules:

- `when` is required and must evaluate to `boolean`.
- Body executes in an implicit child group repeatedly while condition is true.

## 9. `<choice>` / `<option>`

Syntax:

```xml
<choice>
  <option text="Attack" when="hp > 0">
    <code>hp = hp - 1;</code>
  </option>
  <option text="Run" once="true">
    <text value="You ran away."/>
  </option>
</choice>
```

`<option>` attributes:

- `text` (required): displayed label (supports `${expr}`).
- `when` (optional): visibility condition.
- `once` (optional, `true|false`): if true, option disappears after first selection.

Rules:

- Unsupported child nodes inside `<choice>` cause compile error.
- Options with `when=false` are hidden.
- Engine can snapshot only while waiting on choice.

## 10. `<call>`

Syntax:

```xml
<call script="combat/buff.script.xml" args="amount:3,target:ref:hp"/>
```

Attributes:

- `script` (required): target script path key.
- `args` (optional): comma-separated argument list.

Argument grammar:

- Value arg: `name:expr`
- Ref arg: `name:ref:path.to.var`

Rules:

- Default is pass-by-value.
- `ref` arguments copy back to caller when callee returns.
- Target script must be registered.
- Tail-position `call` may compact stack automatically.

## 11. `<return>`

Normal return:

```xml
<return/>
```

Transfer return:

```xml
<return script="next/scene.script.xml"/>
```

Rules:

- `<return/>` returns to caller continuation.
- `<return script="..."/>` switches execution to target script root group and does not return to current script.

## 12. Execution and Snapshot Notes

- Runtime is group-stack based (implicit groups), not script-name based.
- `next()`:
  - advances execution,
  - returns `text`, `choices`, or `end`.
- `choose(index)` selects current choice option.
- `snapshot()` is valid only when `waitingChoice === true`.
- `resume(snapshot)` requires compatible schema and compiler version.

## 13. Common Authoring Errors

1. Using removed nodes (`set/push/remove`) -> compile error.
2. Declaring duplicate vars -> compile error.
3. Missing required attrs (`when`, `script`, `text`) -> compile error.
4. Condition not boolean at runtime -> runtime error.
5. Writing wrong type in `<code>` -> runtime type mismatch error.
6. Writing `undefined` -> runtime error.

