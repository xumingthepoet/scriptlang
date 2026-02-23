# ScriptLang Syntax Manual (V3)

This manual defines the concrete XML authoring syntax for ScriptLang V3.

## 1. File and Root

- File extensions:
  - `.script.xml` for executable scripts
  - `.types.xml` for global custom type declarations
  - `.json` for global read-only data assets
- Root elements:
  - `<script name="...">`
  - `<types name="...">`

Example:

```xml
<!-- include: gamestate.types.xml -->
<script name="main" args="number:hp">
  <text>HP is ${hp}</text>
</script>
```

## 2. Include Header Directives

ScriptLang supports include directives in file header comments:

```xml
<!-- include: shared.types.xml -->
<!-- include: combat.script.xml -->
<!-- include: config.json -->
<script name="main">...</script>
```

Rules:

- Format is exactly `<!-- include: rel/path.ext -->`.
- One include per line.
- Allowed in both `.script.xml` and `.types.xml`.
- Include paths are resolved relative to the current file path.
- Include graph traversal starts from the `.script.xml` file whose root is `<script name="main">`.
- Only files reachable from that main include closure are compiled.
- Reachable file roots are:
  - `.script.xml` -> `<script>`
  - `.types.xml` -> `<types>`
  - `.json` -> strict JSON payload (`JSON.parse`)
- Custom type and JSON-global visibility is scoped per script file: a script can use only assets reachable from that script's own include closure (transitive).
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

- `type:name`
- `ref:type:name`
- comma-separated

Example:

```xml
<script name="buff" args="number:amount,ref:number:target">
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

## 7.1 JSON Global Symbols

When a reachable included file ends with `.json`, ScriptLang injects it as a global read-only symbol.

Rules:

- Symbol name is the file basename without `.json`.
  - `x.json -> x`
  - `config/player.json -> player`
- Symbol name must be a valid JS identifier (`[A-Za-z_$][A-Za-z0-9_$]*`).
- Duplicate symbol names across reachable JSON files are compile errors.
- JSON is parsed with strict `JSON.parse` (comments and trailing commas are invalid).
- JSON symbols are visible only to scripts that can reach the JSON file in their own include closure.
- JSON symbols are fully read-only at runtime (both `x = ...` and `x.a.b = ...` are rejected).

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
- Builtin `random()` is available:
  - signature is strictly `random()` (zero arguments only),
  - return value is a `uint32` integer in `[0, 4294967295]`,
  - any non-zero arity call is a runtime error.
- `Math.random` remains host VM behavior and is not rewritten by ScriptLang.

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
<call script="buff" args="3,ref:hp"/>
```

Rules:

- `script` is required and refers to target script `name`.
- Call args are optional, positional, and map by target script arg declaration order.
- Call arg form is `[ref:]value`.
- For a target param declared `ref:...`, caller must pass `ref:value`.
- For a target param not declared `ref:...`, caller must not pass `ref:`.
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

Transfer return with args:

```xml
<return script="nextScene" args="1,player.name"/>
```

Rules:

- `args` is optional and positional, mapped by target script arg declaration order.
- Return args are value-only in V1.
- Using `ref:` in return args is a compile error.

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
12. Using `null` as a declared type (`type="null"` or `args="null:x"`) -> compile error (`TYPE_PARSE_ERROR`).
13. Using `value` attribute on `<text>/<code>` -> compile error (`XML_ATTR_NOT_ALLOWED`).
14. Leaving `<text>/<code>` inline content empty -> compile error (`XML_EMPTY_NODE_CONTENT`).
15. Using `ref:` in `<return script="..." args="..."/>` -> compile error (`XML_RETURN_REF_UNSUPPORTED`).
16. Including malformed JSON data -> compile error (`JSON_PARSE_ERROR`).
17. JSON basename is not a valid identifier -> compile error (`JSON_SYMBOL_INVALID`).
18. Duplicate JSON symbol basename across reachable files -> compile error (`JSON_SYMBOL_DUPLICATE`).
