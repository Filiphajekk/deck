import { APP_NAME } from "../lib/branding";

export function App() {
  return (
    <div className="flex h-full items-center justify-center bg-neutral-900 text-neutral-200">
      <h1 className="text-2xl font-semibold">{APP_NAME}</h1>
    </div>
  );
}
