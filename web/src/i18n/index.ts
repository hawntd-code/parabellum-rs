import en from './en';
import es, { type Strings } from './es';

export type Lang = 'es' | 'en';
export type { Strings };

export function getStrings(lang: Lang): Strings {
  return lang === 'en' ? en : es;
}
