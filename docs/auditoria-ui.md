# Auditoría de la interfaz (0.32)

Pedido de Juan (2026-10-19): que cada vista, formulario y ventana quede pulcra, centrada, coherente y
animada. Revisado con capturas de las 53 escenas en tema oscuro y claro (el guion recorre la ventana en
modo demostración). Se marca cada punto al corregirlo.

## Errores (algo no funciona o se ve roto)

- [x] **El arranque podía cortarse**: `init()` empezaba antes de cargar `hostear.js`; si los datos
      llegaban antes, la lista de Servidores fallaba (`savedInfo is not defined`) y no se cargaban
      amigos ni mensajes. Ahora arranca cuando están todos los archivos.
- [x] **Campos de texto casi invisibles en tema oscuro**: `--field` (#1f1d25) es casi igual a
      `--panel` (#1e1d23). Afecta a Agregar servidor, Agregar amigo, Añadir cuenta, Nuevo grupo, Ajustes
      de la instancia, Perfil, Recursos y Java.
- [x] **Doctor**: el botón «Arreglar lo recomendado» se sale de la ventana.
- [x] **Ventanas largas** (Exportar, Crear servidor): los botones de abajo quedan fuera de la vista hasta
      bajar.
- [x] **Contenido de la instancia**: el interruptor de un mod con actualización queda corrido (las
      columnas no alinean).
- [x] **Mundos**: sale «Actualizar todo», que no aplica a mundos.
- [x] **Menú de una instancia** (clic derecho): sale arriba a la izquierda, encima del logo, y no donde
      se hizo clic.
- [x] **Instalar** (ventana genérica): si no hay botón secundario, queda uno vacío.

## Consistencia (cada cosa con una sola forma)

- [x] **Listas desplegables nativas** (`<select>`): flecha y menú del sistema, distintas del resto
      (Explorar, Proyecto, Crear servidor, Reportar, Agregar servidor, Pregenerar).
- [x] **Casillas y opciones**: casillas del sistema (Instalar, Nuevo grupo, Exportar). (Los círculos de
      las categorías de Explorar eran de la demostración: con Modrinth salen sus iconos.)
- [x] **Cabeceras de las ventanas**: unas con X para cerrar y otras sin ella (Agregar amigo, Añadir
      cuenta, Nuevo grupo); unas con «Cerrar» y otras con «Cancelar».
- [ ] **Botones de una fila**: «Aceptar» relleno y «Rechazar» sin fondo; botones redondos junto a
      rectangulares (carpeta en Contenido).
- [ ] **Errores técnicos en los avisos**: algunos muestran el error de JavaScript tal cual.
- [x] Servidor guardado: dice «Modpack» aunque sea una instancia vanilla.

## Diseño y orden

- [ ] **Amigos**: el buscador flota a la mitad, la solicitud y el campo de arriba ocupan media página y
      Grupos queda lejos; ordenar en dos columnas claras.
- [ ] **Panel de amigos** (derecha): tu estado («Jugando… mundo abierto en…») se parte en tres líneas;
      hacerlo una tarjeta con tu cabeza.
- [x] **Nuevo grupo**: «Amigos» queda pegado a la foto; el nombre sin campo visible.
- [ ] **Detalles de la instancia**: una lista de datos sola en mucho espacio vacío.
- [ ] **Ajustes › Acerca de**: sin identidad (logo, versión, enlaces).
- [ ] **Ajustes › Java**: filas sin contraste.
- [ ] **Servidor hosteado**: las tarjetas de números tienen mucho vacío abajo (ahí van las minigráficas).
- [ ] **Explorar**: «Instalar en» corta el nombre de la instancia.

## Movimiento (lo que quedaba del «Resto del pulido»)

- [ ] Esqueletos mientras carga (Explorar, Proyecto, Comunidades).
- [ ] Botones que se hunden al pulsar, igual en todos.
- [ ] Cambio de tema con un círculo que crece desde el botón.
- [ ] Números del servidor que cambian suave y minigráficas de jugadores, procesador y memoria.
- [ ] Entradas coherentes: ventanas, menús, avisos, listas y pestañas con las mismas curvas.

## Revisar después de corregir

- [ ] Inglés: textos que no caben o quedan sin traducir.
- [ ] Ventana chica (960 × 620): nada cortado ni encimado.
- [ ] Teclado: foco visible en todo y orden lógico con Tab.
- [ ] Ventana real (WebView2): las mismas escenas, sin errores en la consola.
