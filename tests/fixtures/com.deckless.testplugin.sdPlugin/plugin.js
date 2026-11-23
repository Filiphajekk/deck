// Minimal Stream Deck plugin for the Deckless integration tests. No dependencies:
// it uses the WebSocket client built into Node.js 22+.
const args = process.argv.slice(2);
const arg = (name) => args[args.indexOf(name) + 1];
const port = arg("-port");
const uuid = arg("-pluginUUID");
const registerEvent = arg("-registerEvent");

const counts = new Map();
const ws = new WebSocket(`ws://127.0.0.1:${port}`);
const send = (msg) => ws.send(JSON.stringify(msg));

function show(context) {
  send({ event: "setTitle", context, payload: { title: String(counts.get(context) ?? 0), target: 0 } });
}

ws.onopen = () => send({ event: registerEvent, uuid });

ws.onmessage = (ev) => {
  const { event, context, payload } = JSON.parse(ev.data);
  switch (event) {
    case "willAppear":
      counts.set(context, payload?.settings?.count ?? 0);
      show(context);
      break;
    case "keyDown": {
      const next = (payload?.settings?.count ?? counts.get(context) ?? 0) + 1;
      counts.set(context, next);
      show(context);
      send({ event: "setSettings", context, payload: { count: next } });
      break;
    }
    case "didReceiveSettings":
      counts.set(context, payload?.settings?.count ?? 0);
      show(context);
      break;
    case "sendToPlugin":
      if (payload?.reset) {
        counts.set(context, 0);
        show(context);
        send({ event: "setSettings", context, payload: { count: 0 } });
        send({ event: "sendToPropertyInspector", action: "com.deckless.testplugin.counter", context, payload: { count: 0 } });
      }
      break;
    default:
      break;
  }
};

ws.onclose = () => process.exit(0);
ws.onerror = () => process.exit(1);
