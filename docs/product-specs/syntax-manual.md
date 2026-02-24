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
6. `<loop>`
7. `<choice>` / `<option>`
8. `<call>`
9. `<return>`

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

Reserved naming:

- `<types name>`, `<type name>`, and `<field name>` values starting with `__` are reserved and rejected with `NAME_RESERVED_PREFIX`.

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
- Names starting with `__` are reserved for internal compiler/macro use and are rejected with `NAME_RESERVED_PREFIX`.

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
- Symbol names starting with `__` are reserved and rejected with `NAME_RESERVED_PREFIX`.
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
- `once` (optional, default `false`):
  - accepts only `"true"` or `"false"`.
  - when `true`, this text node is emitted only once per script for the engine instance.
  - once-state is persisted by snapshot/resume.

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
- Builtin `random(n)` is available:
  - signature is strictly `random(n)` (exactly one argument),
  - `n` must be a positive integer,
  - return value is an integer in `[0, n-1]`,
  - wrong arity is a runtime error (`ENGINE_RANDOM_ARITY`),
  - invalid `n` value/type is a runtime error (`ENGINE_RANDOM_ARG`).
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
  <if when="hp == 3"><continue/></if>
  <if when="hp == 1"><break/></if>
</while>
```

Rules:

- `when` is required and must evaluate to `boolean`.
- `<break/>` exits the nearest enclosing `<while>`.
- `<continue/>` inside while body skips to the nearest enclosing `<while>` next condition check.
- Using `<break/>` outside while is a compile error.
- Using `<continue/>` outside while is a compile error unless it is a direct child of `<option>` (see below).

## 11.1 `<loop>`

Syntax:

```xml
<loop times="3">
  <text>tick</text>
</loop>
```

Rules:

- `times` is required.
- `times` uses regular expression syntax (same style as `when` / `<code>` expressions).
- `times` does not support `${...}` wrapper syntax.
- `<loop>` is compile-time macro sugar and expands to existing runtime primitives (`<var>` + `<while>` + `<code>`).
- `<break/>` and `<continue/>` in loop body follow the same nearest-loop behavior as while.

## 12. `<choice>` / `<option>`

Syntax:

```xml
<choice text="Choose your action (${random(100)})">
  <option text="Attack" when="hp > 0" once="true">
    <code>hp = hp - 1;</code>
    <continue/>
  </option>
  <option text="Run" when="hp > 0">
    <text>You ran away.</text>
  </option>
  <option text="Leave" fall_over="true">
    <text>Nothing else is available.</text>
  </option>
</choice>
```

`<choice>` attributes:

- `text` (required)
  - missing is a compile error (`XML_MISSING_ATTR`)
  - empty/whitespace-only is a compile error (`XML_EMPTY_ATTR`)
  - supports `${expr}` runtime interpolation (same rendering behavior as option text)
  - is host-facing choice prompt text; it is not emitted as `<text>` output

`<option>` attributes:

- `text` (required)
- `when` (optional)
- `once` (optional, default `false`)
  - accepts only `"true"` or `"false"`.
  - once-selected options become unavailable for later selections in the same script.
- `fall_over` (optional, default `false`)
  - accepts only `"true"` or `"false"`.
  - per choice, at most one option can use `fall_over="true"`.
  - fall-over option must be the last option.
  - fall-over option cannot declare `when`.
  - fall-over option is shown only when no non-fall-over option is currently visible.

`<option>` body rules:

- A direct child `<continue/>` returns execution to the current `<choice>` node and prompts selection again.
- If a direct `<continue/>` is used with `once="true"`, the once effect still applies before re-entering the choice.

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
2. Missing required attributes (`name/type/when/script/choice text`) -> compile error.
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
13. Using empty/whitespace-only `text` attribute on `<choice>` -> compile error (`XML_EMPTY_ATTR`).
14. Invalid boolean literal on `once`/`fall_over` -> compile error (`XML_ATTR_BOOL_INVALID`).
15. `fall_over="true"` used more than once in a choice -> compile error (`XML_OPTION_FALL_OVER_DUPLICATE`).
16. `fall_over="true"` not on the last option -> compile error (`XML_OPTION_FALL_OVER_NOT_LAST`).
17. `fall_over="true"` with `when` -> compile error (`XML_OPTION_FALL_OVER_WHEN_FORBIDDEN`).
18. `<break/>` outside `<while>` -> compile error (`XML_BREAK_OUTSIDE_WHILE`).
19. `<continue/>` outside `<while>` and non-option-direct position -> compile error (`XML_CONTINUE_OUTSIDE_WHILE_OR_OPTION`).
20. Using `${...}` wrapper in `<loop times="...">` -> compile error (`XML_LOOP_TIMES_TEMPLATE_UNSUPPORTED`).
21. Using `value` attribute on `<text>/<code>` -> compile error (`XML_ATTR_NOT_ALLOWED`).
22. Leaving `<text>/<code>` inline content empty -> compile error (`XML_EMPTY_NODE_CONTENT`).
23. Using `ref:` in `<return script="..." args="..."/>` -> compile error (`XML_RETURN_REF_UNSUPPORTED`).
24. Including malformed JSON data -> compile error (`JSON_PARSE_ERROR`).
25. JSON basename is not a valid identifier -> compile error (`JSON_SYMBOL_INVALID`).
26. Duplicate JSON symbol basename across reachable files -> compile error (`JSON_SYMBOL_DUPLICATE`).
27. Using `__` prefix in user-defined names (script/arg/var/types-collection/type/field/json symbol) -> compile error (`NAME_RESERVED_PREFIX`).
