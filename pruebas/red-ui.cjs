// Ejecuta el controlador real de Explorar: respaldo, paginación y respuestas fuera de orden.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const app = fs.readFileSync('ui/app.js', 'utf8');
const source = app.match(/^const exploreSource = .*$/m)[0];
const search = app.slice(app.indexOf('async function runExplore('), app.indexOf('const modpackInstalled ='));
const entering = app.slice(app.indexOf('function enterExplore()'), app.indexOf('/** Cambiar de tema'));
const nodes = new Map();
const events = [];
const e = { kind: 'modpack', source: 'curseforge', query: 'ATM', categories: new Set(['cf:1']), loader: '', version: '', sort: 'relevance', hits: [], requestId: 0 };
const context = vm.createContext({
  state: { explore: e },
  $: id => { if (!nodes.has(id)) nodes.set(id, { classList: { add() {} }, innerHTML: '', disabled: false }); return nodes.get(id); },
  skeleton: { results: () => 'cargando' }, exploreTarget: () => null,
  renderExploreResults: from => events.push(['render', from]), renderExploreFilters() {}, loadCategories() {}, renderDrive() {},
  fillTargetSelect() {}, loadExploreVersions: () => events.push(['versions']),
  toast: text => events.push(['toast', text]), selectExploreKind: kind => { e.kind = kind; ++e.requestId; events.push(['kind', kind]); },
  window: { errorRecovery: { panel: error => String(error) } },
});
vm.runInContext(source + '\n' + search + '\n' + entering, context);
const run = append => { context.append = append; return vm.runInContext('runExplore(append)', context); };
(async () => {
  e.initialized = true;
  vm.runInContext('enterExplore()', context);
  assert.ok(events.some(event => event[0] === 'versions'));
  context.invoke = async () => ({ source: 'curseforge', hits: [], total: 0 });
  await run(false);
  assert.equal(e.kind, 'modpack'); // Cero resultados no significa caída del proveedor.
  e.hits = [{ id: 'cf:viejo' }];
  context.invoke = async () => ({ source: 'modrinth', hits: [{ id: 'exacto' }], total: 1, fallback: 'respaldo MR' });
  await run(true);
  assert.equal(e.source, 'curseforge'); // El respaldo no cambia la preferencia guardada.
  assert.equal(e.resolvedSource, 'modrinth');
  assert.equal(e.categories.size, 0);
  assert.deepEqual(e.hits.map(h => h.id), ['exacto']); // No mezcla páginas ni ids de proveedores.
  context.invoke = async () => { throw new Error('429 espera'); };
  await run(false);
  assert.equal(e.kind, 'modpack');
  let finish;
  context.invoke = () => new Promise(resolve => { finish = resolve; });
  const stale = run(false);
  ++e.requestId;
  e.hits = [{ id: 'nuevo' }];
  finish({ source: 'drive', hits: [], total: 0 });
  await stale;
  assert.equal(e.kind, 'modpack');
  assert.equal(e.hits[0].id, 'nuevo');
  context.invoke = async () => ({ source: 'drive', hits: [], total: 0, fallback: 'catálogo Drive' });
  await run(false);
  assert.equal(e.kind, 'drive');
  assert.equal(e.query, 'ATM');
  console.log('OK: búsqueda vacía, respaldo sin mezclar páginas, preferencia, 429, respuesta antigua y Drive');
})().catch(error => { console.error(error); process.exitCode = 1; });
