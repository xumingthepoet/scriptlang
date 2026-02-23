# ScriptLang Syntax Manual (V3)

This manual defines the concrete XML authoring syntax for ScriptLang V3.

## 1. File and Root

- File extensions:
  - `.script.xml` for executable scripts
  - `.types.xml` for global custom type declarations
- Root elements:
  - `<script name="...">`
  - `<types name="...">`

Example:

```xml
<!-- include: gamestate.types.xml -->
<script name="main" args="hp:number">
  <text>HP is ${hp}</text>
</script>
```

## 2. Include Header Directives

ScriptLang supports include directives in file header comments:

```xml
<!-- include: shared.types.xml -->
<!-- include: combat.script.xml -->
<script name="main">...</script>
```

Rules:

- Format is exactly `<!-- include: rel/path.xml -->`.
- One include per line.
- Allowed in both `.script.xml` and `.types.xml`.
- Include paths are resolved relative to the current file path.
- Include graph traversal starts from the `.script.xml` file whose root is `<script name="main">`.
- Only files reachable from that main include closure are compiled.
- Custom type visibility is scoped per script file: a script can use only types reachable from that script's own include closure (transitive).
- Include cycles and missing include targets are compile errors.

## 3. Script Top-Level Structure

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

## 4. Types File Structure

`<types>` attributes:

- `name` (required): metadata label for this type collection.

Children:

- `<type name="TypeName">`
  - `<field name="fieldName" type="TypeExpr"/>`

Example:

```xml
<types name="gamestate">
  <type name="Fighter">
    <field name="hp" type="number"/>
    <field name="moves" type="string[]"/>
  </type>
  <type name="BattleState">
    <field name="player" type="Fighter"/>
    <field name="enemy" type="Fighter"/>
  </type>
</types>
```

## 5. Script Identity and Parameters

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

## 6. `<var>` Declarations

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

## 7. Type Syntax

Supported type expressions:

- Primitive: `number`, `string`, `boolean`
- Array: `T[]`
- Map: `Map<string, T>`
- Custom object type: `TypeName` (declared in `.types.xml` reachable from the current script's include closure)

## 8. `<text>`

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

## 9. `<code>`

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

## 10. `<if>` / `<else>`

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

## 11. `<while>`

Syntax:

```xml
<while when="hp > 0">
  <code>hp = hp - 1;</code>
</while>
```

Rules:

- `when` is required and must evaluate to `boolean`.

## 12. `<choice>` / `<option>`

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

## 13. `<call>`

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

## 14. `<return>`

Normal return:

```xml
<return/>
```

Transfer return:

```xml
<return script="nextScene"/>
```

## 15. XML Escaping Note

XML attribute values still require escaping `<` as `&lt;`.

Example:

```xml
<if when="a &lt; b">
  <text>ok</text>
</if>
```

## 16. Common Authoring Errors

1. Using removed nodes (`vars/step/set/push/remove`) -> compile error.
2. Missing required attributes (`name/type/when/script`) -> compile error.
3. Unknown/invalid include target -> compile error.
4. Include cycle -> compile error.
5. Duplicate type name or duplicate field name -> compile error.
6. Unknown custom type reference, or a type not visible from current script include closure -> compile error.
7. Recursive custom type reference -> compile error.
8. Calling unknown script ID -> runtime error.
9. Ref mode mismatch with script param declaration -> runtime error.
10. Condition not boolean at runtime -> runtime error.
11. Writing wrong type, `undefined`, or `null` into declared script variables -> runtime error.
12. Using `null` as a declared type (`type="null"` or `args="x:null"`) -> compile error (`TYPE_PARSE_ERROR`).
13. Using `value` attribute on `<text>/<code>` -> compile error (`XML_ATTR_NOT_ALLOWED`).
14. Leaving `<text>/<code>` inline content empty -> compile error (`XML_EMPTY_NODE_CONTENT`).
