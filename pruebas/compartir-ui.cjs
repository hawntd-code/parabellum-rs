// Regresión del chat real: una importación pendiente nunca debe anunciar que se puede jugar.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const app = fs.readFileSync('ui/app.js', 'utf8');
const parsing = app.slice(app.indexOf('const SHARE_PREFIX ='), app.indexOf('function shareCardHtml'));
const installing = app.slice(app.indexOf('async function installShared('), app.indexOf('async function messageFriend('));
const events = [];
const context = vm.createContext({
  state: { instances: [], chatMessages: [], page: 'chat' },
  t: es => es, newJob: () => 'prueba', renderMessages() {}, reloadInstances: async () => {},
  toast: (...args) => events.push(['toast', ...args]), navigate: (...args) => events.push(['navigate', ...args]),
  curseforgeAssistant: { open: async id => events.push(['assistant', id]) },
});
vm.runInContext(parsing + installing, context);
const recipe = { n: 'ATM10SKY', mc: '1.21.1', l: 'NeoForge', s: 'curseforge', p: 'cf:1298402', v: 'cf:9261952' };
const message = 'parabellum:instancia2:' + JSON.stringify(recipe) + '\nParabellum 0.39.1+ · actualiza para instalar / update to install';
context.message = message;
assert.equal(vm.runInContext('parseShare(message).unavailable', context), false);
context.state.instances = [{ id: 'vieja', modpackProject: recipe.p, modpackVersion: 'cf:otra' }];
assert.equal(vm.runInContext('sharedOwned(parseShare(message))', context), undefined);
context.state.instances.push({ id: 'exacta', modpackProject: recipe.p, modpackVersion: recipe.v });
assert.equal(vm.runInContext('sharedOwned(parseShare(message)).id', context), 'exacta');
context.old = 'parabellum:instancia:' + JSON.stringify({ ...recipe, s: 'mods', x: 2 });
assert.equal(vm.runInContext('parseShare(old).unavailable', context), true);
context.old = context.old.replace('"x":2', '"x":0');
assert.equal(vm.runInContext('parseShare(old).unavailable', context), true);
context.bad = 'parabellum:instancia2:' + JSON.stringify({ ...recipe, v: 'cf:0' });
assert.equal(vm.runInContext('parseShare(bad).unavailable', context), true);

(async () => {
  context.state.chatMessages = [{ id: 'm', text: message }];
  context.invoke = async () => ({ id: 'nueva', manual: [{ fileId: 1 }, { fileId: 2 }] });
  await vm.runInContext('installShared("m")', context);
  assert.ok(events.some(e => e[0] === 'assistant' && e[1] === 'nueva'));
  assert.ok(events.some(e => e[0] === 'toast' && e[1].includes('Faltan 2')));
  assert.ok(!events.some(e => /verificada|puedes jugar/i.test(e[1])));
  events.length = 0;
  context.invoke = async () => ({ id: 'nueva', manual: [], warning: 'Falta un archivo' });
  await vm.runInContext('installShared("m")', context);
  assert.ok(events.some(e => e[0] === 'toast' && e[1] === 'Falta un archivo' && e[2] === 'error'));
  assert.ok(!events.some(e => /verificada|puedes jugar/i.test(e[1])));
  events.length = 0;
  context.invoke = async () => ({ id: 'nueva', manual: [] });
  await vm.runInContext('installShared("m")', context);
  assert.ok(events.some(e => e[0] === 'toast' && e[1].includes('Instalación verificada')));
  console.log('OK: origen exacto, versión distinta, recetas antiguas, inválidas, pendientes, avisos y lista verificada');
})().catch(error => { console.error(error); process.exitCode = 1; });
