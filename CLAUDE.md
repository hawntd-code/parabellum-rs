# Parabellum: notas para agentes

Launcher de Minecraft no premium en Rust + Tauri v2. Es un solo proyecto para Windows y Mac, y dos
personas trabajan en paralelo, cada una con su agente:

- **Juan** (Windows, dueño del repo): lo compartido y Windows. Publica las versiones en Drive.
- **Diego** (@diegovc20, Mac con chip Apple): la versión de Mac, en ramas `mac/…` con PR a `main`.

Todo en español: commits, PR, comentarios y documentación. Los nombres del código van en inglés. Los
textos visibles van en los dos idiomas: `t!("español", "English")` en Rust y `t("español", "English")`
en `ui/`.

## Al empezar

1. Actualiza `main` (`git pull`), o tu rama sobre `main`.
2. Lee [docs/mac-cambios.md](docs/mac-cambios.md): ahí están los cambios de un lado que le tocan al
   otro y los pendientes de cada revisión.
3. La guía de cada sistema: [docs/mac.md](docs/mac.md) y [docs/windows.md](docs/windows.md). La
   historia y los planes están en [ROADMAP.md](ROADMAP.md). Hacia dónde va Parabellum (cliente oficial
   del servidor, comunidades, soporte, mundos en Drive) y en qué orden: [docs/plan-diciembre.md](docs/plan-diciembre.md).

## Reglas

- Lo que depende del sistema va en `src-tauri/src/plataforma/windows/` y `plataforma/macos/`, con el
  contrato en `plataforma/mod.rs`. Si cambias el contrato, agrégalo en las dos carpetas (aunque sea
  con un `TODO(mac)`).
- **Si cambias código compartido en `main`** (lo que hace Juan): en el mismo commit, agrega una
  entrada en `docs/mac-cambios.md` › «Cambios de Windows» con qué tiene que hacer Mac (o «nada»).
- **Si se publica una versión de Windows en Drive** (la publique Juan o su agente): en ese momento,
  agrega arriba en `docs/mac-cambios.md` › «Versiones publicadas en Drive» el número, la fecha y qué
  cambió para quien juega, y súbelo a `main`. Lo pidió Diego.
- **Si terminas un PR de Mac:** marca en `docs/mac-cambios.md` los pendientes que resolviste. Si
  Windows tiene que compilar o probar algo, ponlo en el PR y en «Pendientes para Windows».
- Nunca subas `config-integrada.json` (las claves), sesiones ni datos de usuarios. El repo es privado.
- Para probar: `cargo test --release` en `src-tauri`. Los comandos `--…` (ver README) se corren con
  `PARABELLUM_ROOT` apuntando a una carpeta de prueba, nunca a la carpeta real de nadie.
