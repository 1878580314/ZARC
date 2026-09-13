import { getContext } from 'svelte';

export const FIELD = Symbol('field');
export interface FieldContext {
  id: string;
  description: () => string | undefined;
  invalid: () => boolean;
}
export const fieldContext = () => getContext<FieldContext | undefined>(FIELD);
