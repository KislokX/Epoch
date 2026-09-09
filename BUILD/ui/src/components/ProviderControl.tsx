interface BoundedControlProps {
  readonly label: string;
  readonly value: number | null;
  readonly min: number;
  readonly max: number;
  readonly step: number;
  readonly onChange: (value: number | null) => void;
}

/**
 * A numeric setting whose bounds are known by the Engine.
 *
 * `null` remains visibly unset instead of silently becoming zero, because the provider's
 * default and an explicit zero are different user decisions.
 */
export function BoundedControl({ label, value, min, max, step, onChange }: BoundedControlProps) {
  const unset = value === null;
  return (
    <div className={`slider${unset ? " slider--unset" : ""}`}>
      <div className="slider__head">
        <span>{label}</span>
        <span className="slider__value">
          {unset ? "provider default" : value}
          {!unset && (
            <button
              type="button"
              className="slider__clear"
              aria-label={`Clear ${label}`}
              title="Back to the provider's default"
              onClick={() => onChange(null)}
            >
              ×
            </button>
          )}
        </span>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        aria-label={label}
        value={value ?? (min + max) / 2}
        onChange={(event) => onChange(Number(event.target.value))}
      />
      <div className="slider__rail">
        <span>{min}</span>
        <span>{max}</span>
      </div>
    </div>
  );
}
