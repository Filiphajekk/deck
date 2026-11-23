/** New random id (same format as the backend: 32 hex chars). */
export function newId(): string {
  return crypto.randomUUID().replaceAll("-", "");
}
