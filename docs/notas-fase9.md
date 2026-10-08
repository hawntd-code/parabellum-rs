# Notas para la fase 9 (social), tomadas de la versión C# retirada

La versión C# (0.4.x) ya tenía amigos, presencia y unirse a mundos, y se probó con usuarios reales.
Su código se retiró el 2026-10-08; esto es lo que conviene reutilizar.

## Mundos LAN/e4mc sin "Invalid session" (servidor de sesión local)

- authlib (Minecraft 1.16+) deja cambiar los servidores de Mojang con propiedades de Java:
  `-Dminecraft.api.env=custom` y `-Dminecraft.api.{session,services,profiles,account,auth}.host=http://127.0.0.1:<puerto>/<tipo>`.
- Parabellum escucha en 127.0.0.1 (puerto fijo 47815 si está libre, para que un juego abierto siga
  funcionando si el launcher se reinicia; si no, uno libre) y:
  - `POST /session/session/minecraft/join` → 204 siempre;
  - `GET /session/session/minecraft/hasJoined?username=X` → 200 con `{id: uuid_offline(X), name: X, properties: []}`
    (204 si el nombre no es válido);
  - todo lo demás se reenvía tal cual (método, cuerpo, Content-Type, Authorization, Accept) a
    sessionserver.mojang.com, api.minecraftservices.com, api.mojang.com o authserver.mojang.com;
    sin internet responde 503.
- Equivale a un servidor offline: cada jugador entra con el UUID offline de su nombre.
  Todos deben abrir el juego desde Parabellum. Registro en `datos/logs/sesion-local.log`.

## e4mc

- Desde e4mc 6 se intenta primero una conexión directa ("Dialtone") que en muchas redes falla con
  "No addressing information available". Antes de abrir el juego, si hay `mods/e4mc*.jar`, poner
  `dialtoneHostEnabled = false` y `dialtonePlayerEnabled = false` en `config/e4mc/e4mc.toml`
  (en los dos lados): así siempre va por el relé, que funciona desde cualquier red.
- Entrar directo a un mundo o servidor: `--quickPlayMultiplayer <dirección>` desde 1.20; antes `--server <host> --port <puerto>`.

## Saber qué hace el jugador (sin mods): leer `logs/latest.log`

- Ignorar el archivo hasta que su fecha sea posterior al arranque (antes es el de la partida anterior);
  leer solo lo nuevo, con el archivo compartido.
- `Starting integrated minecraft server` → mundo propio; `Stopping singleplayer server` o `]: Stopping server` → nada.
- Línea con `Server Connector` y `Connecting to ([^,\s]+), (\d+)` → en un servidor (omitir el puerto si es 25565);
  `Disconnecting from server` / `Disconnected from server` → nada.
- `Started serving on (\d+)` → mundo abierto en LAN en ese puerto (anunciar IP local del adaptador con puerta de enlace).
- `[CHAT]` con un dominio `*.e4mc.link` (sin contar e4mc.link, broker., natives., test.) → dirección pública;
  `[CHAT]` con "no longer publicly hosted" o "dejado de compartir" → ya no.

## Cuenta de Google

- OAuth para escritorio con PKCE (S256) y redirección a `http://127.0.0.1:<puerto libre>/`, `prompt=select_account`.
  Alcances: `openid email profile https://www.googleapis.com/auth/drive.appdata`.
- La app OAuth es **Interna**: solo cuentas @uabc.edu.mx.
- El token de sesión se guardaba cifrado con DPAPI (usuario de Windows) en `datos/sesion-google.bin`.
- Perfil en la carpeta oculta de la app en el Drive del jugador (`perfil-lanzador.json`, parents `appDataFolder`).

## Firebase (proyecto parabellum52)

- Entrar con el id_token de Google: `POST https://identitytoolkit.googleapis.com/v1/accounts:signInWithIdp?key=<firebaseApiKey>`
  con `{postBody: "id_token=<token>&providerId=google.com", requestUri: "http://localhost", returnSecureToken: true}`
  → `localId` (uid), `idToken` (1 h), `refreshToken` (renovar en `securetoken.googleapis.com/v1/token?key=`).
- Realtime Database por REST (`?auth=<idToken>`): GET/PUT/PATCH multirruta (atómico; null borra) y escucha con
  Server-Sent Events (`Accept: text/event-stream`), reconectando si se corta o caduca el token.
  Hora del servidor: `{".sv": "timestamp"}`.
- Estructura (reglas en `firebase-reglas.json`):
  - `names/{nombre en minúsculas}` = uid (un nombre por cuenta; reservar libera el anterior)
  - `users/{uid}` = {name, uuid}
  - `requests/{destino}/{origen}` = {name, at}; si el otro ya te la envió, se hacen amigos directamente
  - `friends/{uid}/{amigo}` = true en las dos direcciones
  - `presence/{uid}` = {name, state (online/playing/offline), pack, packFolder, mc, world (single/server),
    server, address (e4mc), lan (IP:puerto), updated}; los vacíos no se escriben. Se publica cada 60 s;
    sin noticias en 3 min = desconectado; al salir se publica "desconectado" al momento.
