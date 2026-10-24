# CurseForge: portal incompleto en la escuela e instancias compartidas

Anotación de Juan del 2026-10-23. Diagnóstico y propuesta previos a la siguiente iteración.
Los cambios de comportamiento de este documento todavía no están implementados.

## Casos disponibles

- `UsefulSlime-neoforge-1.21-1.12.1.jar`: proyecto 417702, archivo 6282838.
- `moreoverlays-1.24.2-mc1.21.1-neoforge.jar`: proyecto 391382, archivo 6981252.
- Ambos están ahora en la instalación local de ATM10SKY 2.2.0, Minecraft 1.21.1 y NeoForge 21.1.250.
  Su origen guardado es `curseforge`, proyecto `cf:1298402`, versión `cf:9261952`.
  Sirve de caso reproducible; Juan todavía no confirmó si era exactamente el pack compartido.

Páginas oficiales: [Useful Slime](https://www.curseforge.com/minecraft/mc-mods/useful-slime/files/6282838)
y [More Overlays Updated](https://www.curseforge.com/minecraft/mc-mods/more-overlays-updated/files/6981252).

## Portal: lo observado y lo que falta comprobar

En la escuela cargaba el HTML sin estilos; al pulsar Descargar no comenzaba la descarga. Esto es
compatible con recursos o destinos bloqueados, pero aún no identifica el dominio o petición fallida.
No se reprodujo desde la red de la escuela en esta revisión.

El portal actual es un WebView que visita directamente CurseForge. El puente de la API solo
transporta JSON; no lleva los estilos, scripts, cookies, redirecciones ni archivos de la página.
`curseforge_manual.rs` valida navegación y descargas contra los dominios oficiales registrados.
Hay que distinguir un rechazo de esa validación de un fallo de la red y de una respuesta del sitio.

Propuesta:

1. Registrar un diagnóstico del portal: host y clase del recurso, estado HTTP, error de transporte y
   destino de la redirección. Omitir cookies, credenciales y parámetros sensibles. Comparar los dos
   archivos en una red normal y en la escuela; confirmar además si los recursos que devuelven HTML
   son páginas de bloqueo en lugar de CSS/JavaScript.
2. Resolver los destinos concretos que fallen. Si se evalúa un puente web, necesita cubrir el recorrido
   completo, mantener la página oficial y probar cookies, CSP, redirecciones y desafíos del sitio.
   No basta servir el HTML. Antes de implementarlo hay que comprobar la viabilidad y las condiciones
   de CurseForge; no se promete que el puente de API actual pueda utilizarse directamente.
3. Mantener como alternativa una caché local de archivos recibidos legítimamente: reutilizarlos por
   huella y tamaño cuando coincidan exactamente, con la recepción manual actual como respaldo.
4. Conservar los pendientes y explicar el fallo. No inventar enlaces CDN ni convertir los archivos
   sin descarga por API en descargas automáticas.

La [opción de distribución de CurseForge](https://support.curseforge.com/support/solutions/articles/9000207877-project-distribution-toggle)
controla el acceso de terceros por API. El problema de red del portal y esa decisión del autor son
condiciones distintas. Referencia adicional: [términos de la API](https://support.curseforge.com/support/solutions/articles/9000207405-curseforge-3rd-party-api-terms-and-conditions).

## Compartir: fallo confirmado en el código

- `compartir::instances_source` solo reconoce `modrinth`, aunque el instalador ya guarda el origen
  y los identificadores de CurseForge en `instancia.json`.
- `compartir::recipe` convierte el resto en `mods`: transmite versiones reconocidas por Modrinth y
  solo cuenta los archivos omitidos con `left_out`. No transmite los overrides del modpack.
- `instalar_compartida` solo distingue Modrinth y catálogo; cualquier otro valor entra en la ruta de
  mods de Modrinth. `installShared` puede anunciar «Ya puedes jugarla» después de una receta parcial.
- Esa receta tampoco identifica el archivo original por su huella: `files_for_versions` reconstruye
  archivos desde la versión. El origen de descarga por sí solo no demuestra corrupción binaria;
  faltantes, configuraciones y variantes perdidas son el problema confirmado de reconstrucción.

Propuesta de comportamiento:

1. **Modpack original:** compartir proveedor, proyecto y archivo exactos. En CurseForge se descarga
   ese mismo ZIP y se usa su instalador actual, con Minecraft, loader, manifiesto y overrides. No se
   convierte en una lista de Modrinth ni se elige automáticamente otra versión si falta la pedida.
2. **Instancia personalizada:** distinguirla del original. Una copia exacta necesita también altas,
   bajas, mods desactivados y configuraciones elegidas, con rutas y huellas. Si la receta no puede
   expresarlo completamente, ofrecer exportar un paquete y explicar lo que incluye, sin presentar
   la reconstrucción parcial como una copia idéntica. Mundos, sesiones y datos personales quedan
   fuera de la selección predeterminada.
3. **Estado completo:** propagar el resultado de importación, sus avisos y archivos manuales hasta
   la interfaz. Mostrar «Faltan 2 archivos» y abrir el asistente; anunciar que está lista solo después
   de verificar lo requerido. Comprobar compatibilidad e integridad antes de habilitar el recorrido
   de jugar de una instalación compartida.
4. **Clientes anteriores:** versionar las recetas nuevas para que un launcher viejo no interprete
   `curseforge` como una instancia vacía de mods. Rechazar fuentes desconocidas. Cubrir la tarjeta
   del chat del launcher y la vista del mod; indicar la actualización necesaria.
5. **Mensajes anteriores:** las recetas ya enviadas sin identidad de CurseForge no permiten recuperar
   con certeza el pack original. Pedir volver a compartir desde el remitente actualizado.

## Pruebas fuera de la escuela, 2026-10-23

Se ejecutaron con el candidato Windows 0.39.0, cuyo SHA-256 es
`b6c6fe782bc228047beced80b860ff020d9b2dcc121c809e3637c6dae9d01b16`, en una raíz de prueba
separada. No se cambiaron la red de Windows, el ejecutable habitual ni las instancias del jugador.

`pruebas/curseforge-red.py` pasó diez comprobaciones:

1. Useful Slime: archivo 6282838, 609.239 bytes y SHA-1 oficial coincidente.
2. More Overlays: archivo 6981252, 112.984 bytes y SHA-1 oficial coincidente.
3. Los dos archivos se conservan como pendientes exactos; no aparecen instalados.
4. Reabrir el proceso y repetir conserva la cola sin duplicados.
5. Respuesta HTTP 403: error y pendientes conservados.
6. Respuesta HTTP 429: error y pendientes conservados.
7. Respuesta HTTP 503: error y pendientes conservados.
8. HTML de bloqueo con estado 200 en lugar del JSON: error y pendientes conservados.
9. API disponible y CDN bloqueado: falla la descarga sin declararla completa.
10. Puente remoto bloqueado: error de conexión y pendientes conservados.

Los metadatos reales del puente indicaron `downloadUrl` vacío para ambos archivos. Para el recorrido
de fallos se usó un servidor local con esos metadatos y dependencias omitidas para aislar cada archivo.
El bloqueo usa un proxy limitado al proceso de prueba y escucha solo en `127.0.0.1`; se confirmó el
intento de conexión a `edge.forgecdn.net` y al puente público. No es una prueba de todo ATM10SKY.

También pasaron las tres pruebas de `curseforge_manual::tests`: nombres y orígenes válidos, rechazo
de contenido incorrecto y recuperación de una entrada completa conservando los pendientes parciales.

Una sonda compilada con `compartir.rs` sin modificar reprodujo el fallo de origen. Con identificación
de archivos simulada (dos jars sin versión de Modrinth), la receta de CurseForge quedó como `mods`,
con dos omitidos y sin proyecto ni versión. Como control, el mismo metadato con origen `modrinth`
sí conserva su identidad. Es una reproducción de la lógica con dependencias simuladas, no el envío
de un chat ni una reinstalación completa del pack.

Repetición de la prueba de red, desde la raíz del repositorio (elige una carpeta de prueba nueva):

```powershell
python pruebas/curseforge-red.py --exe src-tauri/target/release/Parabellum.exe --root ../_prueba-cf-red-nueva --mods "../Parabellum/instancias/All the Mods 10_ To the Sky   ATM10SKY/mods"
```

**Límite:** estas comprobaciones no ejercitan CSS/JavaScript del portal en el WebView, ni reproducen
la política exacta de la escuela. Falta identificar el recurso o destino que falla allí y hacer la
validación final desde esa red. Los bloqueos HTTP/CDN y la pérdida de origen pueden probarse aquí.
Los resultados locales están en `_verificacion-curseforge-red-03/resultados.json` y en
`_verificacion-curseforge-red-02/receta.json` y `recepcion-unitarias.txt`, fuera del repositorio.

## Orden y aceptación

Primero corregir compartir y el estado de instalación, que tienen causas verificadas. En paralelo de
planificación, preparar el diagnóstico del portal con los dos mods indicados; elegir el transporte
cuando haya evidencia del fallo en la escuela. Ninguna de estas incidencias se marca resuelta por las
pruebas anteriores de Drive de la 0.39.0.

- [ ] Receta de CurseForge conserva proyecto/archivo exactos y no invoca Modrinth.
- [ ] Destinatario nuevo instala también los overrides; cliente antiguo rechaza la receta nueva.
- [ ] Instalación incompleta conserva los dos pendientes y no anuncia «Ya puedes jugarla».
- [ ] Instancia personalizada no pierde silenciosamente modificaciones ni archivos.
- [ ] Portal de ambos mods: CSS, JavaScript y descarga verificados desde la escuela.
- [ ] Recepción valida huella/tamaño, coloca cada archivo y reanuda la instalación.
- [ ] Pruebas de Windows y Mac; revisión nativa de los recorridos modificados.
