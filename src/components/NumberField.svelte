<script lang="ts">
  // A number input with large − and + buttons for touch.

  interface Props {
    label: string;
    value: number | null;
    min?: number;
    max?: number;
    placeholder?: string;
    /** Allow fractions, such as fade times. Otherwise typed values are rounded. */
    fractional?: boolean;
    onchange?: (value: number | null) => void;
  }

  let {
    label,
    value = $bindable(),
    min = 0,
    max = Number.MAX_SAFE_INTEGER,
    placeholder,
    fractional = false,
    onchange,
  }: Props = $props();

  function set(next: number | null) {
    value = next == null ? null : Math.max(min, Math.min(max, next));
    onchange?.(value);
  }

  function step(delta: number) {
    const base = value ?? (placeholder && !Number.isNaN(Number(placeholder)) ? Number(placeholder) : min);
    set(value == null && delta > 0 ? base : base + delta);
  }

  const round = (n: number) => (fractional ? Math.round(n * 100) / 100 : Math.round(n));

  function input(event: Event) {
    const text = (event.target as HTMLInputElement).value.trim();
    set(text === "" ? null : Number.isFinite(Number(text)) ? round(Number(text)) : value);
  }
</script>

<label class="field">
  {label}
  <span class="number">
    <button type="button" aria-label="Decrease {label}" onclick={() => step(-1)}>−</button>
    <input
      type="text"
      inputmode={fractional ? "decimal" : "numeric"}
      aria-label={label}
      value={value ?? ""}
      {placeholder}
      onchange={input}
    />
    <button type="button" aria-label="Increase {label}" onclick={() => step(1)}>+</button>
  </span>
</label>

<style>
  .number {
    display: grid;
    grid-template-columns: var(--touch) 1fr var(--touch);
    gap: 6px;
  }

  button {
    padding: 0;
    font-size: 1.4rem;
  }

  input {
    text-align: center;
    font-variant-numeric: tabular-nums;
    color: var(--text);
  }
</style>
