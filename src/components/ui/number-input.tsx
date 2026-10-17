import { useState } from "react";
import type { InputHTMLAttributes } from "react";

type NumberInputProps = Omit<
  InputHTMLAttributes<HTMLInputElement>,
  "value" | "type"
> & {
  value: number;
};

export function NumberInput({
  value,
  onChange,
  onBlur,
  ...props
}: NumberInputProps) {
  const [edit, setEdit] = useState({ source: value, text: String(value) });
  if (edit.source !== value) {
    setEdit({ source: value, text: String(value) });
  }
  return (
    <input
      {...props}
      type="number"
      value={edit.text}
      onChange={(event) => {
        const text = event.currentTarget.value;
        setEdit({ source: value, text });
        if (text !== "" && Number.isFinite(event.currentTarget.valueAsNumber)) {
          onChange?.(event);
        }
      }}
      onBlur={(event) => {
        setEdit({ source: value, text: String(value) });
        onBlur?.(event);
      }}
    />
  );
}
