# Portal de CurseForge

Función HTTP `portal_curseforge`, Python 3.14. Servicio independiente del puente de API.
Permite exclusivamente los hosts enumerados en `main.py`, puerto 443. El WebView inicia
el TLS con CurseForge: el relevo recibe bytes cifrados, nunca cookies ni páginas en claro.
No ofrece enlaces de archivos ni convierte una restricción de API en descarga automática.

Despliegue: máximo **una instancia**, concurrencia **20**, timeout **60 s** y variable
`WORKERS=1` y `THREADS=20` (variables de Functions Framework). Las conexiones viven en memoria y no se
comparten entre procesos. Un cambio de revisión cierra conexiones; se reabre el portal.
Máximo 8 conexiones TCP simultáneas, 15 minutos y 128 MiB por conexión. Cada una necesita
solicitudes HTTP de envío y recepción; el límite de 20 solicitudes también restringe el servicio.
Una conexión abandonada se libera tras 60 segundos sin actividad, aunque el cliente haya terminado
antes de confirmar el cierre. Los cuerpos HTTP/1 y HTTP/2 se leen con el mismo límite de 64 KiB.
No guarda contenido
ni identificadores de sesión en los logs de aplicación. Cloud Run registra ruta y estado
HTTP; los identificadores viajan en cabeceras, nunca en las URL.

No es un proxy general ni modifica la red del equipo. `/health` identifica el protocolo.
Su disponibilidad y los dominios auxiliares del portal deben validarse en cada red.
