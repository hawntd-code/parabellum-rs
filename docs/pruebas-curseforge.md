# Verificación de CurseForge 0.36.0

2026-10-22. Código de la entrega: `9d1d5d4`. Windows x64, con el puente de Cloud Run ya publicado.

- `cargo test --release`: **125 correctas, 0 fallidas, 1 omitida**. La omitida es el túnel P2P por internet,
  ajeno a esta integración. Las pruebas nuevas cubren compatibilidad de archivos elegidos a mano,
  manifiestos y loaders, URLs restringidas, reconocimiento por SHA-1 y rutas de los overrides.
- `cargo build --release`: ejecutable 0.36.0, sin advertencias. Se comprobó que no contiene la clave
  privada de CurseForge que hay en la configuración local.
- Todos los archivos JavaScript pasan `node --check`.
- [Windows en GitHub](https://github.com/hawntd-code/parabellum-rs/actions/runs/39882080060):
  compilación con advertencias prohibidas y pruebas correctas.
- [Mac en GitHub](https://github.com/hawntd-code/parabellum-rs/actions/runs/39882082922):
  compilación y pruebas correctas. Esto no sustituye la prueba de la interfaz en una Mac real.

## Recorrido con archivos reales

`pruebas/curseforge.py` pasó sus **9 comprobaciones**, en una carpeta aparte y con copias de la base:

1. El puente público devuelve 52 categorías, sin clave local.
2. El ejecutable busca Architectury en CurseForge.
3. Instala Architectury para Fabric 1.21.1 con Fabric API y verifica el SHA-1 de ambos.
4. Reconoce la versión ya instalada sin repetirla.
5. Rechaza ese archivo cuando la instancia usa Minecraft 1.20.1.
6. Si el autor retira el enlace externo (respuesta simulada con el mismo archivo real), ofrece su
   página y no descarga el jar.
7. Prepara la base de prueba mediante copias, sin enlaces a las instancias reales.
8. Importa un zip de CurseForge: crea la instancia, instala Fabric, descarga dos mods, copia overrides
   y guarda los índices de procedencia e integridad.
9. Los mods importados coinciden con las huellas de CurseForge.

Además, la instancia importada llegó al **menú principal en 65 s** con Architectury 13.0.11,
Fabric API 0.116.17 y el panel de Parabellum. La observación del arranque se permitió hasta 150 s.

Desde el catálogo real se instaló **Fabulously Optimized 6.5.0** (`cf:396246`, archivo `cf:8217274`),
Fabric 1.21.1, en 27 s. El importador informó correctamente dos descargas manuales: `nuit-interop`
y `entityculling`. No se forzaron sus enlaces ni se dio por completo ese modpack sin esos archivos.

En la ventana real de Windows se comprobaron el selector Modrinth/CurseForge, categorías, búsqueda,
la marca «Instalado», descripción del proyecto, enlace a CurseForge y versiones compatibles.

## Confirmación en la red de la escuela

El 2026-10-22 Juan confirmó que CurseForge funciona correctamente desde la escuela tras hacer sus
pruebas. Confirmó también la publicación de la 0.36.1 en Drive y la subida del modpack ligero.
Queda medir la descarga del nuevo ligero y comprobar su instalación desde una carpeta nueva.

## Pendientes

- Interfaz e instalación en una Mac real antes de publicar una versión de Mac.
- Favoritos y actualizaciones automáticas del contenido de CurseForge, para otra entrega.
