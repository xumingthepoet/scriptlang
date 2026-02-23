# ScriptLang Syntax Manual (V2)

This manual defines the concrete XML authoring syntax for ScriptLang V2.

## 1. File and Root

- File extension: `.script.xml`.
- Exactly one script per file.
- Root element must be `<script>`.
- `<script name="...">` is the runtime script ID.

Example:

```xml
<script name="main" args="hp:number">
  <text>HP is ${hp}</text>
</script>
```

## 2. Top-Level Structure

Allowed direct children of `<script>` are executable nodes:

1. `<var>`
2. `<text>`
3. `<code>`
4. `<if>` / `<else>`
5. `<while>`
6. `<choice>` / `<option>`
7. `<call>`
8. `<return>`

Removed nodes (compile-time error):

- `<vars>`
- `<step>`
- `<set>`
- `<push>`
- `<remove>`

## 3. Script Identity and Parameters

`<script>` attributes:

- `name` (required): unique script ID for runtime lookup and calls.
- `args` (optional): parameter declaration list.

`args` grammar:

- `name:type`
- `name:type:ref`
- comma-separated

Example:

```xml
<script name="buff" args="amount:number,target:number:ref">
  <code>target = target + amount;</code>
</script>
```

Rules:

- `name` must be unique across compiled scripts.
- `args` defines script-root typed variables.
- Missing call arguments use type-based default values.

## 4. `<var>` Declarations

Syntax:

```xml
<var name="hp" type="number" value="10"/>
```

Attributes:

- `name` (required)
- `type` (required)
- `value` (optional)

Rules:

- `<var>` is executable and takes effect at declaration point.
- Scope is declaration point to the end of current block.
- Current block means one of:
  - script body
  - if/else branch body
  - while body
  - option body
- Exiting the block drops that variable.
- `undefined` is not allowed.

## 5. Type Syntax

Supported type expressions:

- Primitive: `number`, `string`, `boolean`
- Array: `T[]`
- Map: `Map<string, T>`

## 6. `<text>`

Allowed form:

```xml
<text>
  HP=${hp}
</text>
```

Interpolation `${expr}` is evaluated at runtime.
Rules:
- `value` attribute is not allowed on `<text>`.
- Inline content must be non-empty (after trim).

## 7. `<code>`

Allowed form:

```xml
<code>hp = hp + 1;</code>
```

Rules:

- Can read/write visible scoped variables.
- Type checks are enforced for declared variables.
- Assignment to `undefined` is rejected.
- `value` attribute is not allowed on `<code>`.
- Inline content must be non-empty (after trim).

## 8. `<if>` / `<else>`

Syntax:

```xml
<if when="hp > 0">
  <text>alive</text>
  <else>
    <text>dead</text>
  </else>
</if>
```

Rules:

- `when` is required and must evaluate to `boolean`.
- `<else>` is optional.

## 9. `<while>`

Syntax:

```xml
<while when="hp > 0">
  <code>hp = hp - 1;</code>
</while>
```

Rules:

- `when` is required and must evaluate to `boolean`.

## 10. `<choice>` / `<option>`

Syntax:

```xml
<choice>
  <option text="Attack" when="hp > 0">
    <code>hp = hp - 1;</code>
  </option>
  <option text="Run">
    <text>You ran away.</text>
  </option>
</choice>
```

`<option>` attributes:

- `text` (required)
- `when` (optional)

## 11. `<call>`

Syntax:

```xml
<call script="buff" args="amount:3,target:ref:hp"/>
```

Rules:

- `script` is required and refers to target script `name`.
- Call args are optional and default to pass-by-value.
- For a target param declared `:ref`, caller must pass `name:ref:path`.
- For a target param not declared `:ref`, caller must not pass `ref`.
- `ref` values copy back when callee returns.

## 12. `<return>`

Normal return:

```xml
<return/>
```

Transfer return:

```xml
<return script="nextScene"/>
```

## 13. XML Escaping Note

XML attribute values still require escaping `<` as `&lt;`.

Example:

```xml
<if when="a &lt; b">
  <text>ok</text>
</if>
```

## 14. Common Authoring Errors

1. Using removed nodes (`vars/step/set/push/remove`) -> compile error.
2. Missing required attributes (`name/type/when/script`) -> compile error.
3. Calling unknown script ID -> runtime error.
4. Ref mode mismatch with script param declaration -> runtime error.
5. Condition not boolean at runtime -> runtime error.
6. Writing wrong type, `undefined`, or `null` into declared script variables -> runtime error.
7. Using `null` as a declared type (`type="null"` or `args="x:null"`) -> compile error (`TYPE_PARSE_ERROR`).
8. Using `value` attribute on `<text>/<code>` -> compile error (`XML_ATTR_NOT_ALLOWED`).
9. Leaving `<text>/<code>` inline content empty -> compile error (`XML_EMPTY_NODE_CONTENT`).
