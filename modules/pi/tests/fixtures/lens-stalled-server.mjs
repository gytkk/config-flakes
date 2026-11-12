// Disposable adversarial LSP: initialize correctly, then never answer queries.
// No filesystem, network, or child processes. Lens must report unavailable/timeout.
let buffer = Buffer.alloc(0);
function send(message) {
  const body = JSON.stringify(message);
  process.stdout.write(`Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`);
}
process.stdin.on("data", chunk => {
  buffer = Buffer.concat([buffer, chunk]);
  while (true) {
    const end = buffer.indexOf("\r\n\r\n");
    if (end < 0) return;
    const length = Number(/Content-Length: (\d+)/i.exec(buffer.subarray(0, end).toString())?.[1]);
    if (!Number.isFinite(length) || buffer.length < end + 4 + length) return;
    const message = JSON.parse(buffer.subarray(end + 4, end + 4 + length).toString());
    buffer = buffer.subarray(end + 4 + length);
    if (message.method === "initialize") send({ jsonrpc: "2.0", id: message.id, result: { capabilities: {
      textDocumentSync: 1, hoverProvider: true, diagnosticProvider: { interFileDependencies: false, workspaceDiagnostics: false },
    } } });
    else if (message.method === "shutdown") send({ jsonrpc: "2.0", id: message.id, result: null });
    else if (message.method === "exit") process.exit(0);
  }
});
