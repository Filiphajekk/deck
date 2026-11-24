/** Backend errors of the form `i18n:<key>` are translated; anything else is shown as is. */
export function errorText(t: (k: string) => string, msg: string): string {
  const m = /^(?:Error: )?i18n:([\w.]+)/.exec(msg);
  return m ? t(m[1]) : msg;
}
