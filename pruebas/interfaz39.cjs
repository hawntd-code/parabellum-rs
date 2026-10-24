// Contratos de recuperación y tarjetas, sin navegador ni datos del usuario.
const fs = require("node:fs");
const vm = require("node:vm");
const assert = require("node:assert/strict");
const root = require("node:path").resolve(__dirname, "..");
const elements = new Map();
const node = () => ({ addEventListener() {}, classList: { add() {}, remove() {}, contains() { return false; } }, style: {} });
const context = vm.createContext({
  console, setTimeout, clearTimeout,
  window: { addEventListener() {}, dispatchEvent() {} }, document: { addEventListener() {} },
  state: { instances: [{ id: "local", name: "Prueba" }], current: null, settings: {} },
  t: (es) => es, icon: (name) => `<svg>${name}</svg>`, escapeHtml: (value) => String(value).replaceAll("<", "&lt;").replaceAll(">", "&gt;"),
  friendlyError: String,
  invoke: () => new Promise(() => {}), listen: () => Promise.resolve(),
  $: (selector) => { if (!elements.has(selector)) elements.set(selector, node()); return elements.get(selector); },
  $$: () => [], community: { id: "c", detail: { id: "c", members: [{ uid: "me", you: true, role: "member" }, { uid: "staff", role: "admin" }] } },
});
vm.runInContext(fs.readFileSync(root + "/ui/acciones.js", "utf8"), context);
const recovery = context.window.errorRecovery;
const text = recovery.capture("mundos_respaldar", { origen: { id: "local", world: "Mundo", kind: "instancia" } }, "Sin conexión ⟦accion:reintentar⟧");
assert.equal(text, "Sin conexión");
assert.equal(recovery.present(text).actions[0].label, "Reintentar");
assert.equal(recovery.present("Desconocido ⟦accion:ejecutar-comando⟧").actions.length, 0);
assert.equal(recovery.present(recovery.capture("enviar_mensaje", { texto: "privado" }, "Sin internet ⟦accion:reintentar⟧")).actions.length, 0);
assert.equal(recovery.present(recovery.capture("contenido", { datos: "x".repeat(20000) }, "No respondió ⟦accion:reintentar⟧")).actions.length, 0);
assert.equal(recovery.present(recovery.capture("contenido", {}, "Descarga cancelada")).actions.length, 0);
assert.ok(!recovery.panel(text).includes("⟦"));
assert.ok(!recovery.present("Mojang bloqueado ⟦accion:escuela⟧").actions.some(action => action.id === "escuela"));
assert.equal(recovery.present(recovery.capture("verificar_instancia", { id: "local" }, "Java no está ⟦accion:recursos⟧ ⟦accion:carpeta⟧")).actions.length, 2);

vm.runInContext(fs.readFileSync(root + "/ui/soporte.js", "utf8"), context);
const run = (code) => vm.runInContext(code, context);
run(`support.open = {community: "c", id: "ticket", ticket: {by: "me"}};
support.settings = [{id: "minecraft.renderDistance", label: "Renderizado", value: 12, instanceName: "Oficial", target: {package: "p", minecraft: "1.21.1", contentHash: "h"}}];
var message = {id: "m", from: "staff", name: "Soporte", card: {v: 1, op: "o", kind: "setting", setting: "minecraft.renderDistance", value: 8, scope: "instancia", official: "c", target: {package: "p", minecraft: "1.21.1", contentHash: "h"}}};`);
assert.ok(run(`cardHtml(message,{})`).includes('data-sp-apply="m"'));
run(`support.applied.o = {result: "pendiente"}`);
assert.ok(run(`cardHtml(message,{})`).includes("Se aplicará al cerrar Minecraft"));
assert.ok(run(`cardHtml(message,{})`).includes('data-sp-undo="o"'));
run(`support.applied.o = {result: "pendiente-deshacer"}`);
assert.ok(run(`cardHtml(message,{})`).includes("Se deshará al cerrar Minecraft"));
run(`support.applied.o = {result: "conflicto"}`);
assert.ok(!run(`cardHtml(message,{})`).includes("data-sp-apply"));
run(`support.applied.o = {result: "aplicada"}`);
assert.ok(run(`cardHtml(message,{})`).includes("Deshacer"));
run(`support.applied = {}; community.detail.members[1].role = "member";`);
assert.ok(!run(`cardHtml(message,{})`).includes("data-sp-apply"));
assert.equal(run(`settingValue("minecraft.fov",90)`), "90°");
assert.equal(run(`settingValue("minecraft.graphics",2)`), "Fabulosos");
run(`message.card = {...message.card, setting: "launcher.school_mode", scope: "launcher", target: null}; support.applied.o = {result: "aplicada"};`);
assert.ok(run(`cardHtml(message,{})`).includes("automáticamente"));
assert.ok(!run(`cardHtml(message,{})`).includes("data-sp-undo"));
assert.ok(!run(`cardHtml(message,{})`).includes("data-sp-apply"));
console.log("Recuperación: marcas ocultas, comandos permitidos, límites y cancelación: OK");
console.log("Tarjetas: aplicar, pendiente, cancelar, deshacer, conflicto y rol actual: OK");
