// Las capas de la excavación, de arriba hacia abajo. El material decide cómo se dibuja cada una
// (src/scripts/pixel.js › MATERIALS) y el ícono es el de la barra rápida.
import type { Strings } from '../i18n';

export const HERO = { id: 'inicio', y: 64 } as const;

export const LAYERS = [
  { id: 'funciones', material: 'dirt', y: 52, icon: 'iso:grassTop|grass' },
  { id: 'amigos', material: 'cave', y: 36, icon: 'sprite:face-you' },
  { id: 'hostear', material: 'gravel', y: 20, icon: 'iso:lampOn|lampOn' },
  { id: 'mods', material: 'redstone', y: 4, icon: 'iso:craftTop|craftSide' },
  { id: 'comunidades', material: 'geode', y: -12, icon: 'sprite:shard' },
  { id: 'escuela', material: 'portal', y: -26, icon: 'sprite:barrier' },
  { id: 'detalles', material: 'deepslate', y: -40, icon: 'sprite:pickaxe' },
  { id: 'descargar', material: 'diamond', y: -54, icon: 'sprite:gem-diamond' },
  { id: 'preguntas', material: 'bedrock', y: -60, icon: 'sprite:book' },
] as const;

export type LayerId = (typeof LAYERS)[number]['id'];

export function layerOf(id: LayerId) {
  const layer = LAYERS.find(item => item.id === id);
  if (!layer) throw new Error(`Capa desconocida: ${id}`);
  return layer;
}

// «Tierra · Y 52»: la etiqueta de profundidad sobre cada título.
export function coordOf(id: LayerId | 'inicio', t: Strings) {
  const y = id === 'inicio' ? HERO.y : layerOf(id).y;
  return `${t.layers[id].biome} · Y ${y}`;
}
