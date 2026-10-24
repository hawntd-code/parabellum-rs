package mx.parabellum.juego;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import javax.imageio.ImageIO;
import java.awt.Color;
import java.awt.Shape;
import java.awt.image.BufferedImage;
import java.io.ByteArrayInputStream;
import java.io.File;
import java.text.SimpleDateFormat;
import java.util.ArrayList;
import java.util.Calendar;
import java.util.Date;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;

/**
 * El panel de Parabellum dentro del juego, con lo mismo que la parte social de la ventana:
 * <ul>
 * <li>Amigos: agregar por nombre, solicitudes, la lista con filtros y búsqueda (Unirse, Invitar, Mensaje,
 * favoritos, eliminar) y los grupos.</li>
 * <li>Mensajes: conversaciones (favoritas primero, quién está en línea, fotos de grupo), responder,
 * reaccionar, editar y borrar, invitaciones e imágenes; crear grupos y, si eres el dueño, cambiarlos.</li>
 * <li>Capturas: mandarlas a una conversación.</li>
 * </ul>
 * Se dibuja entero con Java2D en unidades de diseño (ver {@link Lienzo}); cada clic se resuelve con las
 * zonas de la última pasada.
 */
final class Panel {
    private static final double W = 880, H = 560, LATERAL = 196, LISTA = 248;
    private static final int AMIGOS = 0, MENSAJES = 1, CAPTURAS = 2;
    private static final int PRIMARIO = 0, NORMAL = 1, PELIGRO = 2, SUAVE = 3;
    static final String COMPARTIR = "parabellum:instancia:";
    static final String COMPARTIR2 = "parabellum:instancia2:";
    static final String INVITACION = "parabellum:invitacion:";
    private static final Color FILA = new Color(0x2a, 0x28, 0x32, 120);
    private static final Color CITA = new Color(0, 0, 0, 46);

    private interface Accion {
        void hacer();
    }

    private static final class Zona {
        final double x, y, w, h;
        final Accion accion;
        final Accion derecho;
        /** Es una opción del menú del clic derecho. */
        boolean deMenu;

        Zona(double x, double y, double w, double h, Accion accion, Accion derecho) {
            this.x = x;
            this.y = y;
            this.w = w;
            this.h = h;
            this.accion = accion;
            this.derecho = derecho;
        }

        boolean dentro(double px, double py) {
            return px >= x && py >= y && px < x + w && py < y + h;
        }

        boolean misma(Zona otra) {
            return otra != null && otra.x == x && otra.y == y && otra.w == w && otra.h == h;
        }
    }

    /** Un diálogo dentro del panel (crear un grupo, confirmar, agregar un amigo…). */
    private static final class Dialogo {
        final String tipo;
        String titulo, texto, ok, error, chat, nota;
        boolean peligro, ocupado;
        Accion accion;
        final Set<String> elegidos = new LinkedHashSet<String>();

        Dialogo(String tipo) {
            this.tipo = tipo;
        }
    }

    private final Juego juego;
    private Imagen imagen;
    private double u = 1;
    private int x, y;
    private boolean sucio = true;
    private volatile boolean recargar;
    private final List<Zona> zonas = new ArrayList<Zona>();
    private double[] recorte = {0, 0, W, H};
    private double ratonX = -1, ratonY = -1;
    private Zona encima;
    /** Mientras se dibuja lo que queda debajo de un diálogo, nada se resalta. */
    private boolean debajo;

    private int pestana = AMIGOS;
    private final Campo agregar = new Campo(T.t("Nombre de jugador de tu amigo", "Your friend's player name"), 16);
    private final Campo buscar = new Campo(T.t("Buscar", "Search"), 16);
    private final Campo escribir = new Campo(T.t("Escribe un mensaje…", "Write a message…"), 2000);
    private final Campo campoDialogo = new Campo("", 40);
    private String filtro = "todos";
    private double scrollAmigos, scrollGrupos, scrollChats, scrollMensajes, scrollCapturas, scrollEnviar, scrollDialogo;
    private double maxAmigos, maxGrupos, maxChats, maxMensajes, maxCapturas, maxEnviar, maxDialogo;
    private double gruposX = W;
    private String chatAbierto;
    private JsonArray mensajes;
    private String editando;
    /** El mensaje al que se responde: {id, name, text, mine}. */
    private JsonObject respondiendo;
    private final Map<String, BufferedImage> fotos = new ConcurrentHashMap<String, BufferedImage>();
    private final Map<String, Boolean> pedidas = new ConcurrentHashMap<String, Boolean>();
    private final Map<String, BufferedImage> fotosGrupo = new ConcurrentHashMap<String, BufferedImage>();
    private final Map<String, Boolean> pedidasGrupo = new ConcurrentHashMap<String, Boolean>();
    private File captura;
    private boolean enviando;
    private List<Object[]> menu;
    private String[] menuEmojis;
    private Accion[] menuEmojiAccion;
    private double menuX, menuY;
    private boolean registrandoMenu;
    private Dialogo dialogo;
    private long parpadeo;
    /** Amigos por uid (de la última pasada) para el punto de en línea de las conversaciones. */
    private Map<String, JsonObject> amigosPorUid = new HashMap<String, JsonObject>();

    Panel(Juego juego) {
        this.juego = juego;
    }

    // ================= Lo que llama la versión del juego

    int x() {
        return x;
    }

    int y() {
        return y;
    }

    /** Vuelve a dibujar si hace falta y devuelve la imagen (en píxeles reales). */
    Imagen dibujar(int anchoVentana, int altoVentana, int escala) {
        double nuevoU = Math.max(0.6, Math.min(2.4, Math.min(anchoVentana * 0.92 / W, altoVentana * 0.9 / H)));
        int ancho = Avisos.alinear((int) Math.round(W * nuevoU), escala);
        int alto = Avisos.alinear((int) Math.round(H * nuevoU), escala);
        x = Avisos.alinear((anchoVentana - ancho) / 2, escala);
        y = Avisos.alinear((altoVentana - alto) / 2, escala);
        if (imagen == null || imagen.ancho != ancho || imagen.alto != alto) {
            imagen = new Imagen(ancho, alto);
            sucio = true;
        }
        u = Math.min((double) ancho / W, (double) alto / H);
        boolean conFoco = agregar.foco || buscar.foco || escribir.foco || campoDialogo.foco;
        long ahora = System.currentTimeMillis();
        if (conFoco && ahora - parpadeo > 530) {
            parpadeo = ahora;
            sucio = true;
        }
        if (recargar) {
            recargar = false;
            sucio = true;
        }
        if (sucio) {
            sucio = false;
            Lienzo l = new Lienzo(imagen.buffer, u);
            zonas.clear();
            pintar(l, (ahora / 530) % 2 == 0);
            l.listo();
            imagen.terminado();
        }
        return imagen;
    }

    void marcar() {
        sucio = true;
    }

    /** Desde otros hilos (imágenes que terminaron de cargar). */
    void marcarLuego() {
        recargar = true;
    }

    void raton(double px, double py) {
        double nx = (px - x) / u, ny = (py - y) / u;
        boolean movio = nx != ratonX || ny != ratonY;
        ratonX = nx;
        ratonY = ny;
        Zona nueva = zonaEn(ratonX, ratonY);
        // Solo se vuelve a dibujar si el ratón pasó a otra zona (o a otra fila de mensajes).
        if ((nueva == null) != (encima == null) || (nueva != null && !nueva.misma(encima)) || (movio && pestana == MENSAJES && filaBajo() != filaEncima)) {
            encima = nueva;
            sucio = true;
        }
    }

    boolean clic(double px, double py, int boton) {
        raton(px, py);
        Zona z = zonaEn(ratonX, ratonY);
        boolean habiaMenu = menu != null || menuEmojis != null;
        cerrarMenu();
        sucio = true;
        if (boton == 1) {
            for (int i = zonas.size() - 1; i >= 0; i--) {
                Zona c = zonas.get(i);
                if (c.dentro(ratonX, ratonY) && c.derecho != null && !c.deMenu) {
                    c.derecho.hacer();
                    break;
                }
            }
            return true;
        }
        // Con el menú abierto, un clic elige una opción o solo lo cierra.
        if (habiaMenu) {
            if (z != null && z.deMenu && z.accion != null) z.accion.hacer();
            return true;
        }
        agregar.foco = false;
        buscar.foco = false;
        escribir.foco = false;
        campoDialogo.foco = false;
        if (z != null && z.accion != null) z.accion.hacer();
        sucio = true;
        return ratonX >= 0 && ratonY >= 0 && ratonX < W && ratonY < H;
    }

    boolean rueda(double px, double py, double cantidad) {
        raton(px, py);
        double paso = cantidad * 48;
        if (dialogo != null) scrollDialogo = limitar(scrollDialogo - paso, maxDialogo);
        else if (pestana == AMIGOS && ratonX >= gruposX) scrollGrupos = limitar(scrollGrupos - paso, maxGrupos);
        else if (pestana == AMIGOS) scrollAmigos = limitar(scrollAmigos - paso, maxAmigos);
        else if (pestana == MENSAJES && ratonX < LATERAL + LISTA) scrollChats = limitar(scrollChats - paso, maxChats);
        else if (pestana == MENSAJES) scrollMensajes = limitar(scrollMensajes + paso, maxMensajes);
        else if (captura != null) scrollEnviar = limitar(scrollEnviar - paso, maxEnviar);
        else scrollCapturas = limitar(scrollCapturas - paso, maxCapturas);
        sucio = true;
        return true;
    }

    private static double limitar(double valor, double maximo) {
        return Math.max(0, Math.min(Math.max(0, maximo), valor));
    }

    private Campo campoActivo() {
        if (campoDialogo.foco) return campoDialogo;
        if (agregar.foco) return agregar;
        if (buscar.foco) return buscar;
        if (escribir.foco) return escribir;
        return null;
    }

    /** Devuelve true si la tecla se usó (si no, la versión del juego la atiende: Esc cierra). */
    boolean tecla(int tecla, int mods) {
        sucio = true;
        if (tecla == Campo.ESCAPE) {
            if (menu != null || menuEmojis != null) {
                cerrarMenu();
                return true;
            }
            if (dialogo != null) {
                cerrarDialogo();
                return true;
            }
            if (respondiendo != null) {
                respondiendo = null;
                return true;
            }
            if (editando != null) {
                editando = null;
                escribir.poner("");
                return true;
            }
            if (captura != null) {
                captura = null;
                return true;
            }
            return false;
        }
        Campo campo = campoActivo();
        if (dialogo != null && (tecla == Campo.ENTER || tecla == Campo.ENTER_NUMERICO)) {
            aceptarDialogo();
            return true;
        }
        if (campo == null) return false;
        if (tecla == Campo.ENTER || tecla == Campo.ENTER_NUMERICO) {
            if (campo == agregar) enviarSolicitud(agregar);
            else if (campo == escribir) enviarMensaje();
            return true;
        }
        if (tecla == 265 && campo == escribir && escribir.valor.isEmpty()) {
            editarUltimo();
            return true;
        }
        return campo.tecla(tecla, mods, juego.plataforma);
    }

    boolean caracter(char c) {
        Campo campo = campoActivo();
        if (campo == null && dialogo != null && conCampo(dialogo)) {
            campoDialogo.foco = true;
            campo = campoDialogo;
        }
        if (campo == null && dialogo == null && pestana == MENSAJES && chatAbierto != null) {
            escribir.foco = true;
            campo = escribir;
        }
        if (campo == null) return false;
        sucio = true;
        boolean usado = campo.caracter(c);
        if (campo == buscar) scrollAmigos = 0;
        return usado;
    }

    /** -Dparabellum.prueba.pestana=1: al abrir, la primera conversación (para probar sin tocar nada). */
    private boolean pruebaHecha, pruebaConectada;

    void abierto() {
        sucio = true;
        // En las pruebas (-Dparabellum.prueba.abrir) se avisa que el panel ya está a la vista.
        if (System.getProperty("parabellum.prueba.abrir") != null) {
            try {
                java.nio.file.Files.write(new File(juego.plataforma.carpeta(), "parabellum-prueba.txt").toPath(), String.valueOf(System.currentTimeMillis()).getBytes("UTF-8"));
            } catch (Exception ignored) {
            }
            // -Dparabellum.prueba.conectar=<dirección>: entra a ese servidor como lo haría «Unirse».
            final String direccion = System.getProperty("parabellum.prueba.conectar");
            if (direccion != null && !pruebaConectada) {
                pruebaConectada = true;
                juego.plataforma.enJuego(() -> {
                    juego.plataforma.cerrarPanel();
                    juego.plataforma.conectar(direccion);
                });
            }
        }
        String prueba = System.getProperty("parabellum.prueba.pestana");
        if (!pruebaHecha && "capturas".equals(prueba)) {
            pruebaHecha = true;
            pestana = CAPTURAS;
        }
        if (!pruebaHecha && ("1".equals(prueba) || "2".equals(prueba))) {
            JsonArray chats = lista(juego.estado, "chats");
            int cual = "2".equals(prueba) ? 1 : 0;
            if (chats.size() > cual) {
                pruebaHecha = true;
                irAChat(texto(chats.get(cual).getAsJsonObject(), "id"));
            }
        }
        if (!pruebaHecha && "grupo".equals(prueba)) {
            pruebaHecha = true;
            pestana = MENSAJES;
            nuevoGrupo();
        }
        if (pestana == MENSAJES && chatAbierto != null) {
            pedirChat(chatAbierto);
            escribir.foco = true;
        }
    }

    void cerrado() {
        agregar.foco = false;
        buscar.foco = false;
        escribir.foco = false;
        campoDialogo.foco = false;
        cerrarMenu();
        // La imagen del panel (8–24 MB según la ventana) se suelta; se vuelve a dibujar al abrirlo.
        imagen = null;
        sucio = true;
        if (chatAbierto != null) juego.puente("cerrar_chat", null, null);
    }

    /** Llegaron los mensajes de la conversación abierta. */
    void mensajes(String chat, JsonArray lista) {
        if (!chat.equals(chatAbierto)) return;
        mensajes = lista;
        sucio = true;
        if (juego.plataforma.panelAbierto()) {
            JsonObject datos = new JsonObject();
            datos.addProperty("chat", chat);
            juego.puente("leido", datos, null);
        }
    }

    boolean viendo(String chat) {
        return juego.plataforma.panelAbierto() && pestana == MENSAJES && chat != null && chat.equals(chatAbierto);
    }

    /** Ir a una conversación (desde un aviso o la lista de amigos). */
    void irAChat(String chat) {
        pestana = MENSAJES;
        captura = null;
        abrirChat(chat);
    }

    // ================= Zonas

    private Zona zonaEn(double px, double py) {
        for (int i = zonas.size() - 1; i >= 0; i--) {
            if (zonas.get(i).dentro(px, py)) return zonas.get(i);
        }
        return null;
    }

    /** Registra una zona dentro del recorte actual. Devuelve true si el ratón está encima. */
    private boolean zona(double zx, double zy, double zw, double zh, Accion accion, Accion derecho) {
        double x0 = Math.max(zx, recorte[0]), y0 = Math.max(zy, recorte[1]);
        double x1 = Math.min(zx + zw, recorte[0] + recorte[2]), y1 = Math.min(zy + zh, recorte[1] + recorte[3]);
        if (x1 <= x0 || y1 <= y0) return false;
        Zona z = new Zona(x0, y0, x1 - x0, y1 - y0, accion, derecho);
        z.deMenu = registrandoMenu;
        zonas.add(z);
        return !debajo && (menu == null && menuEmojis == null || registrandoMenu) && z.dentro(ratonX, ratonY);
    }

    private boolean sobre(double zx, double zy, double zw, double zh) {
        return !debajo && menu == null && menuEmojis == null && ratonX >= Math.max(zx, recorte[0]) && ratonY >= Math.max(zy, recorte[1])
            && ratonX < Math.min(zx + zw, recorte[0] + recorte[2]) && ratonY < Math.min(zy + zh, recorte[1] + recorte[3]);
    }

    private void recortar(Lienzo l, double rx, double ry, double rw, double rh) {
        recorte = new double[] {rx, ry, rw, rh};
        l.g.setClip(l.forma(rx, ry, rw, rh, 0));
    }

    private void sinRecorte(Lienzo l) {
        recorte = new double[] {0, 0, W, H};
        l.g.setClip(null);
    }

    // ================= Piezas

    private static double anchoBoton(Lienzo l, String texto, String icono) {
        return l.ancho(texto, 12.5, true) + (icono != null ? 42 : 24);
    }

    /** Botón con texto (e icono a la izquierda, si lo tiene). Devuelve su ancho. */
    private double boton(Lienzo l, String texto, String icono, double bx, double by, double h, int estilo, Accion accion) {
        double w = anchoBoton(l, texto, icono);
        boolean sobre = zona(bx, by, w, h, accion, null) && accion != null;
        Color fondo = estilo == PRIMARIO ? (sobre ? Lienzo.ACENTO_ENCIMA : Lienzo.ACENTO) : estilo == SUAVE ? (sobre ? Lienzo.TARJETA_ENCIMA : new Color(0, 0, 0, 0)) : (sobre ? Lienzo.TARJETA_ENCIMA : Lienzo.TARJETA);
        if (accion == null && estilo == PRIMARIO) fondo = new Color(0x7f, 0x50, 0xf2, 110);
        l.rect(bx, by, w, h, 8, fondo);
        Color letra = estilo == PRIMARIO ? Color.WHITE : estilo == PELIGRO ? Lienzo.PELIGRO : Lienzo.TEXTO;
        double tx = bx + 12;
        if (icono != null) {
            Iconos.dibujar(l, icono, bx + 11, by + (h - 15) / 2, 15, letra);
            tx += 18;
        }
        l.texto(texto, tx, by + (h - l.altoLinea(12.5)) / 2, 12.5, true, letra);
        return w;
    }

    /** Botón alineado a la derecha: devuelve su x. */
    private double botonDerecha(Lienzo l, String texto, String icono, double derecha, double by, double h, int estilo, Accion accion) {
        double w = anchoBoton(l, texto, icono);
        boton(l, texto, icono, derecha - w, by, h, estilo, accion);
        return derecha - w;
    }

    /** Botón cuadrado con solo un icono. */
    private void botonIcono(Lienzo l, String icono, double bx, double by, double d, int estilo, Accion accion) {
        boolean sobre = zona(bx, by, d, d, accion, null);
        Color fondo = estilo == PRIMARIO ? (sobre ? Lienzo.ACENTO_ENCIMA : Lienzo.ACENTO) : estilo == SUAVE ? (sobre ? Lienzo.TARJETA_ENCIMA : new Color(0, 0, 0, 0)) : (sobre ? Lienzo.TARJETA_ENCIMA : Lienzo.TARJETA);
        l.rect(bx, by, d, d, Math.min(9, d / 2), fondo);
        double t = Math.round(d * 0.56);
        Color color = estilo == PRIMARIO ? Color.WHITE : estilo == PELIGRO ? Lienzo.PELIGRO : sobre ? Lienzo.TEXTO : Lienzo.APAGADO;
        Iconos.dibujar(l, icono, bx + (d - t) / 2, by + (d - t) / 2, t, color);
    }

    /** Número sobre una pestaña o una conversación. */
    private void insignia(Lienzo l, String texto, double derecha, double cy, boolean acento) {
        double w = Math.max(20, l.ancho(texto, 11, true) + 12);
        l.rect(derecha - w, cy - 10, w, 20, 10, acento ? Lienzo.ACENTO : Lienzo.TARJETA_ENCIMA);
        l.texto(texto, derecha - w + (w - l.ancho(texto, 11, true)) / 2, cy - 8, 11, true, Color.WHITE);
    }

    /** Cabeza con el punto de en línea (verde) o jugando (morado), como en Parabellum. */
    private void cabezaAmigo(Lienzo l, String nombre, JsonObject amigo, double ax, double ay, double d, Color fondo) {
        l.cabeza(nombre, ax, ay, d);
        boolean enLinea = amigo != null && booleano(amigo, "enLinea");
        if (amigo != null && !enLinea) l.rect(ax, ay, d, d, d * 0.18, new Color(0x1e, 0x1d, 0x23, 140));
        if (enLinea) {
            double p = Math.max(9, d * 0.32);
            l.circulo(ax + d - p + 3, ay + d - p + 3, p, fondo);
            l.circulo(ax + d - p + 5, ay + d - p + 5, p - 4, booleano(amigo, "jugando") ? Lienzo.ACENTO : Lienzo.EXITO);
        }
    }

    /** La foto del grupo, o el icono sobre morado suave. */
    private void avatarGrupo(Lienzo l, JsonObject chat, double ax, double ay, double d) {
        BufferedImage foto = fotoGrupo(chat);
        if (foto != null) {
            l.imagen(foto, ax, ay, d, d, d * 0.26);
            return;
        }
        l.rect(ax, ay, d, d, d * 0.26, Lienzo.ACENTO_SUAVE);
        double t = Math.round(d * 0.52);
        Iconos.dibujar(l, "users", ax + (d - t) / 2, ay + (d - t) / 2, t, Lienzo.ACENTO_TEXTO);
    }

    private void avatarChat(Lienzo l, JsonObject chat, double ax, double ay, double d, Color fondo) {
        if (esGrupo(chat)) avatarGrupo(l, chat, ax, ay, d);
        else cabezaAmigo(l, texto(chat, "nombre"), amigosPorUid.get(texto(chat, "amigo")), ax, ay, d, fondo);
    }

    private void etiqueta(Lienzo l, String texto, double ex, double ey) {
        l.texto(texto.toUpperCase(), ex, ey, 10.5, true, Lienzo.TENUE);
    }

    private void vacio(Lienzo l, double cx, double cy, String icono, String texto, String detalle, Object[] boton) {
        if (icono != null) {
            l.rect(cx - 26, cy - 72, 52, 52, 16, Lienzo.ACENTO_SUAVE);
            Iconos.dibujar(l, icono, cx - 13, cy - 59, 26, Lienzo.ACENTO_TEXTO);
        }
        double ty = cy - 8;
        for (String linea : l.lineas(texto, 380, 14, true)) {
            l.texto(linea, cx - l.ancho(linea, 14, true) / 2, ty, 14, true, Lienzo.TEXTO);
            ty += 20;
        }
        if (detalle != null) {
            for (String linea : l.lineas(detalle, 380, 12.5, false)) {
                l.texto(linea, cx - l.ancho(linea, 12.5, false) / 2, ty + 2, 12.5, false, Lienzo.APAGADO);
                ty += 18;
            }
        }
        if (boton != null) {
            double w = anchoBoton(l, (String) boton[0], (String) boton[2]);
            boton(l, (String) boton[0], (String) boton[2], cx - w / 2, ty + 14, 34, PRIMARIO, (Accion) boton[1]);
        }
    }

    // ================= Dibujo

    private void pintar(Lienzo l, boolean cursorVisible) {
        l.rect(0, 0, W, H, 14, Lienzo.FONDO);
        Shape antes = l.g.getClip();
        l.g.clip(l.forma(0, 0, W, H, 14));
        l.rect(0, 0, LATERAL, H, 0, Lienzo.LATERAL);
        l.g.setClip(antes);
        l.borde(0, 0, W, H, 14, Lienzo.BORDE);
        amigosPorUid = new HashMap<String, JsonObject>();
        for (JsonElement e : lista(juego.estado, "amigos")) {
            JsonObject a = e.getAsJsonObject();
            if (texto(a, "uid") != null) amigosPorUid.put(texto(a, "uid"), a);
        }
        debajo = dialogo != null;
        pintarLateral(l);
        JsonObject estado = juego.estado;
        double cx = LATERAL + (W - LATERAL) / 2;
        if (!juego.conectado()) {
            vacio(l, cx, H / 2, "alert", juego.puenteDisponible() ? T.t("Esperando a Parabellum…", "Waiting for Parabellum…") : T.t("Abre este juego desde Parabellum", "Open this game from Parabellum"),
                T.t("Aquí ves a tus amigos, les escribes y mandas capturas sin salir del juego.", "Here you see your friends, write to them and send screenshots without leaving the game."), null);
        } else if (!booleano(estado, "sesion")) {
            vacio(l, cx, H / 2, "user", T.t("Inicia sesión con Google en Parabellum", "Sign in with Google in Parabellum"),
                T.t("Así ves a tus amigos y entras a sus mundos con un clic.", "So you can see your friends and join their worlds with one click."),
                new Object[] {T.t("Abrir Parabellum", "Open Parabellum"), (Accion) () -> juego.puente("abrir_parabellum", null, null), null});
        } else if (pestana == AMIGOS) {
            pintarAmigos(l, cursorVisible);
        } else if (pestana == MENSAJES) {
            pintarMensajes(l, cursorVisible);
        } else {
            pintarCapturas(l);
        }
        sinRecorte(l);
        debajo = false;
        if (dialogo != null) pintarDialogo(l, cursorVisible);
        if (menu != null || menuEmojis != null) pintarMenu(l);
    }

    private void pintarLateral(Lienzo l) {
        JsonObject estado = juego.estado;
        l.texto("parabellum", 20, 18, 18, true, Lienzo.ACENTO_TEXTO);
        String yo = texto(objeto(estado, "yo"), "nombre");
        boolean sesion = juego.conectado() && booleano(estado, "sesion");
        l.rect(12, 52, LATERAL - 24, 52, 10, Lienzo.TARJETA);
        l.cabeza(yo == null ? "Jugador" : yo, 22, 61, 34);
        l.texto(l.recortar(yo == null ? "Jugador" : yo, LATERAL - 92, 13.5, true), 66, 60, 13.5, true, Lienzo.TEXTO);
        l.circulo(66, 82, 7, sesion ? Lienzo.EXITO : Lienzo.TENUE);
        l.texto(sesion ? T.t("En línea", "Online") : T.t("Sin sesión", "Not signed in"), 78, 78, 11.5, false, Lienzo.APAGADO);

        int sinLeer = 0;
        for (JsonElement c : lista(estado, "chats")) if (booleano(c.getAsJsonObject(), "sinLeer")) sinLeer++;
        int solicitudes = lista(estado, "solicitudes").size();
        pestanaBoton(l, AMIGOS, "users", T.t("Amigos", "Friends"), 122, solicitudes > 0 ? String.valueOf(solicitudes) : null);
        pestanaBoton(l, MENSAJES, "chat", T.t("Mensajes", "Messages"), 164, sinLeer > 0 ? (sinLeer > 9 ? "9+" : String.valueOf(sinLeer)) : null);
        pestanaBoton(l, CAPTURAS, "image", T.t("Capturas", "Screenshots"), 206, null);

        boolean conectado = juego.conectado();
        l.circulo(20, H - 47, 7, conectado ? Lienzo.EXITO : Lienzo.PELIGRO);
        l.texto(conectado ? T.t("Conectado a Parabellum", "Connected to Parabellum") : T.t("Sin Parabellum", "No Parabellum"), 33, H - 51, 11.5, false, Lienzo.APAGADO);
        l.texto(l.recortar(juego.plataforma.descripcion() + " · " + T.t("Esc para cerrar", "Esc to close"), LATERAL - 36, 10.5, false), 20, H - 30, 10.5, false, Lienzo.TENUE);
    }

    private void pestanaBoton(Lienzo l, final int cual, String icono, String nombre, double by, String cuenta) {
        boolean activa = pestana == cual;
        boolean sobre = zona(10, by, LATERAL - 20, 36, () -> cambiarPestana(cual), null);
        if (activa || sobre) l.rect(10, by, LATERAL - 20, 36, 9, activa ? Lienzo.ACENTO_SUAVE : FILA);
        Iconos.dibujar(l, icono, 22, by + 9, 18, activa ? Lienzo.ACENTO_TEXTO : Lienzo.APAGADO);
        l.texto(nombre, 50, by + 9, 13.5, true, activa ? Lienzo.TEXTO : Lienzo.APAGADO);
        if (cuenta != null) insignia(l, cuenta, LATERAL - 20, by + 18, true);
    }

    private void cambiarPestana(int cual) {
        pestana = cual;
        cerrarMenu();
        captura = cual == CAPTURAS ? captura : null;
        if (cual == MENSAJES && chatAbierto != null) {
            pedirChat(chatAbierto);
            escribir.foco = true;
        }
    }

    private void titulo(Lienzo l, String texto, String detalle) {
        double w = l.texto(texto, LATERAL + 24, 18, 20, true, Lienzo.TEXTO);
        if (detalle != null) l.texto(detalle, LATERAL + 24 + w + 10, 25, 12, false, Lienzo.TENUE);
    }

    // ---------- Amigos

    private void pintarAmigos(Lienzo l, boolean cursorVisible) {
        JsonArray amigos = lista(juego.estado, "amigos");
        int enLinea = 0, jugando = 0;
        for (JsonElement e : amigos) {
            JsonObject a = e.getAsJsonObject();
            if (booleano(a, "enLinea")) enLinea++;
            if (booleano(a, "enLinea") && booleano(a, "jugando")) jugando++;
        }
        titulo(l, T.t("Amigos", "Friends"), amigos.size() > 0 ? T.t(enLinea + " de " + amigos.size() + " en línea", enLinea + " of " + amigos.size() + " online") : null);
        double x0 = LATERAL + 24, ancho = W - x0 - 24;

        // Agregar por nombre
        double campoW = 260;
        zona(x0, 56, campoW, 34, () -> agregar.foco = true, null);
        agregar.dibujar(l, x0, 56, campoW, 34, cursorVisible && agregar.foco);
        boton(l, T.t("Enviar solicitud", "Send request"), "send", x0 + campoW + 8, 56, 34, PRIMARIO, () -> enviarSolicitud(agregar));

        double ty = 104;
        JsonArray solicitudes = lista(juego.estado, "solicitudes");
        for (JsonElement e : solicitudes) {
            final JsonObject s = e.getAsJsonObject();
            final String uid = texto(s, "uid");
            final String nombre = texto(s, "nombre");
            l.rect(x0, ty, ancho, 48, 10, Lienzo.ACENTO_SUAVE);
            l.cabeza(nombre, x0 + 10, ty + 9, 30);
            l.texto(l.recortar(nombre, 260, 13.5, true), x0 + 50, ty + 8, 13.5, true, Lienzo.TEXTO);
            l.texto(T.t("Quiere ser tu amigo", "Wants to be your friend"), x0 + 50, ty + 26, 11.5, false, Lienzo.APAGADO);
            double bx = botonDerecha(l, T.t("Rechazar", "Decline"), null, x0 + ancho - 9, ty + 9, 30, NORMAL, () -> accionAmigo("rechazar", uid, null));
            botonDerecha(l, T.t("Aceptar", "Accept"), null, bx - 6, ty + 9, 30, PRIMARIO, () -> accionAmigo("aceptar", uid, T.t("Ahora son amigos.", "You are now friends.")));
            ty += 54;
        }
        if (solicitudes.size() > 0) ty += 6;

        // Grupos a la derecha
        double gw = 206, gx = x0 + ancho - gw;
        gruposX = gx - 8;
        double mw = gx - 18 - x0;
        pintarGrupos(l, gx, ty, gw);

        // Filtros y búsqueda
        String[][] filtros = {{"todos", T.t("Todos", "All"), String.valueOf(amigos.size())}, {"linea", T.t("En línea", "Online"), String.valueOf(enLinea)}, {"jugando", T.t("Jugando", "Playing"), String.valueOf(jugando)}};
        double fx = x0;
        double totalFiltros = 6;
        for (String[] f : filtros) totalFiltros += l.ancho(f[1], 12, true) + (f[2].equals("0") ? 0 : l.ancho(f[2], 12, true) + 5) + 22;
        l.rect(x0, ty, totalFiltros, 32, 16, Lienzo.CAMPO);
        fx += 3;
        for (final String[] f : filtros) {
            double fw = l.ancho(f[1], 12, true) + (f[2].equals("0") ? 0 : l.ancho(f[2], 12, true) + 5) + 22;
            boolean activo = filtro.equals(f[0]);
            boolean sobre = zona(fx, ty + 3, fw, 26, () -> {
                filtro = f[0];
                scrollAmigos = 0;
            }, null);
            if (activo) l.rect(fx, ty + 3, fw, 26, 13, Lienzo.TARJETA_ENCIMA);
            double tw = l.texto(f[1], fx + 11, ty + 9, 12, true, activo || sobre ? Lienzo.TEXTO : Lienzo.APAGADO);
            if (!f[2].equals("0")) l.texto(f[2], fx + 11 + tw + 5, ty + 9, 12, true, Lienzo.TENUE);
            fx += fw;
        }
        double bw = Math.min(150, mw - totalFiltros - 10);
        if (bw > 80) {
            zona(x0 + mw - bw, ty, bw, 32, () -> buscar.foco = true, null);
            buscar.dibujar(l, x0 + mw - bw, ty, bw, 32, cursorVisible && buscar.foco);
        }
        ty += 42;

        // La lista: favoritos, jugando, en línea y desconectados.
        double arriba = ty, abajo = H - 12;
        recortar(l, x0 - 4, arriba, mw + 8, abajo - arriba);
        double fy = arriba - scrollAmigos;
        List<JsonObject> visibles = new ArrayList<JsonObject>();
        String q = buscar.valor.trim().toLowerCase(Locale.ROOT);
        for (JsonElement e : amigos) {
            JsonObject a = e.getAsJsonObject();
            boolean online = booleano(a, "enLinea");
            if ("linea".equals(filtro) && !online) continue;
            if ("jugando".equals(filtro) && !(online && booleano(a, "jugando"))) continue;
            String n = texto(a, "nombre");
            if (!q.isEmpty() && (n == null || !n.toLowerCase(Locale.ROOT).contains(q))) continue;
            visibles.add(a);
        }
        java.util.Collections.sort(visibles, (a, b) -> {
            int ra = rango(a), rb = rango(b);
            if (ra != rb) return ra - rb;
            return String.valueOf(texto(a, "nombre")).compareToIgnoreCase(String.valueOf(texto(b, "nombre")));
        });
        if (visibles.isEmpty()) {
            String vacio = amigos.size() == 0 ? T.t("Todavía no tienes amigos en Parabellum. Escribe arriba el nombre de jugador de un amigo.", "You have no friends on Parabellum yet. Type a friend's player name above.")
                : !q.isEmpty() ? T.t("Nadie coincide con la búsqueda.", "No one matches the search.")
                : "jugando".equals(filtro) ? T.t("Nadie está jugando ahora.", "No one is playing right now.") : T.t("Nadie está en línea ahora.", "No one is online right now.");
            for (String linea : l.lineas(vacio, mw - 16, 13, false)) {
                l.texto(linea, x0 + 4, fy + 8, 13, false, Lienzo.APAGADO);
                fy += 19;
            }
        }
        boolean puedoInvitar = booleano(juego.estado, "puedoInvitar");
        for (final JsonObject a : visibles) {
            pintarAmigo(l, a, x0, fy, mw, puedoInvitar);
            fy += 52;
        }
        maxAmigos = Math.max(0, fy + scrollAmigos - abajo + 8);
        scrollAmigos = limitar(scrollAmigos, maxAmigos);
        sinRecorte(l);
    }

    private static int rango(JsonObject a) {
        if (booleano(a, "favorito")) return 0;
        if (!booleano(a, "enLinea")) return 3;
        return booleano(a, "jugando") ? 1 : 2;
    }

    private void pintarAmigo(Lienzo l, final JsonObject a, double fx, double fy, double fw, boolean puedoInvitar) {
        final String uid = texto(a, "uid");
        final String nombre = texto(a, "nombre");
        boolean online = booleano(a, "enLinea");
        boolean sobreFila = zona(fx, fy, fw, 48, () -> mensajeA(uid), () -> menuAmigo(a));
        if (sobreFila || sobre(fx, fy, fw, 48)) l.rect(fx, fy, fw, 48, 10, Lienzo.TARJETA);
        cabezaAmigo(l, nombre, a, fx + 8, fy + 8, 32, sobreFila || sobre(fx, fy, fw, 48) ? Lienzo.TARJETA : Lienzo.FONDO);
        double derecha = fx + fw - 8;
        botonIcono(l, "more", derecha - 28, fy + 10, 28, SUAVE, () -> menuAmigo(a));
        derecha -= 32;
        botonIcono(l, "chat", derecha - 28, fy + 10, 28, SUAVE, () -> mensajeA(uid));
        derecha -= 34;
        if (booleano(a, "unirse")) derecha = botonDerecha(l, T.t("Unirse", "Join"), "play", derecha, fy + 10, 28, PRIMARIO, () -> unirse(uid, null)) - 6;
        else if (puedoInvitar && online) derecha = botonDerecha(l, T.t("Invitar", "Invite"), "send", derecha, fy + 10, 28, NORMAL, () -> invitar(uid)) - 6;
        double textoW = derecha - fx - 52;
        double nw = l.texto(l.recortar(nombre, textoW - 18, 13.5, true), fx + 50, fy + 7, 13.5, true, online ? Lienzo.TEXTO : Lienzo.APAGADO);
        if (booleano(a, "favorito")) Iconos.dibujar(l, "star", fx + 50 + nw + 5, fy + 9, 12, new Color(0xe8a33d));
        l.texto(l.recortar(estadoAmigo(a), textoW, 11.5, false), fx + 50, fy + 26, 11.5, false, Lienzo.APAGADO);
    }

    private void pintarGrupos(Lienzo l, double gx, double gy, double gw) {
        etiqueta(l, T.t("Grupos", "Groups"), gx + 4, gy + 9);
        botonIcono(l, "plus", gx + gw - 28, gy + 2, 28, SUAVE, this::nuevoGrupo);
        double arriba = gy + 38, abajo = H - 12;
        recortar(l, gx - 4, arriba, gw + 8, abajo - arriba);
        double ty = arriba - scrollGrupos;
        List<JsonObject> grupos = new ArrayList<JsonObject>();
        for (JsonElement e : lista(juego.estado, "chats")) if (esGrupo(e.getAsJsonObject())) grupos.add(e.getAsJsonObject());
        if (grupos.isEmpty()) {
            for (String linea : l.lineas(T.t("Crea un grupo para hablar con varios amigos a la vez.", "Create a group to talk with several friends at once."), gw - 8, 12, false)) {
                l.texto(linea, gx + 4, ty + 2, 12, false, Lienzo.APAGADO);
                ty += 17;
            }
        }
        for (final JsonObject g : grupos) {
            final String id = texto(g, "id");
            boolean sobre = zona(gx, ty, gw, 44, () -> irAChat(id), null);
            if (sobre) l.rect(gx, ty, gw, 44, 9, Lienzo.TARJETA);
            avatarGrupo(l, g, gx + 7, ty + 7, 30);
            boolean nuevo = booleano(g, "sinLeer");
            l.texto(l.recortar(texto(g, "nombre"), gw - 64, 13, true), gx + 45, ty + 6, 13, true, Lienzo.TEXTO);
            String ultimo = vistaPrevia(texto(g, "ultimo"));
            if (texto(g, "ultimoNombre") != null && !ultimo.isEmpty()) ultimo = texto(g, "ultimoNombre") + ": " + ultimo;
            if (ultimo.isEmpty()) ultimo = T.t(lista(g, "miembros").size() + " miembros", lista(g, "miembros").size() + " members");
            l.texto(l.recortar(ultimo, gw - 64, 11, false), gx + 45, ty + 23, 11, false, nuevo ? Lienzo.TEXTO : Lienzo.APAGADO);
            if (nuevo) l.circulo(gx + gw - 16, ty + 18, 8, Lienzo.ACENTO);
            ty += 48;
        }
        maxGrupos = Math.max(0, ty + scrollGrupos - abajo + 8);
        scrollGrupos = limitar(scrollGrupos, maxGrupos);
        sinRecorte(l);
    }

    private static String estadoAmigo(JsonObject a) {
        if (!booleano(a, "enLinea")) {
            double visto = numero(a, "visto", 0);
            return visto > 0 ? T.t("Desconectado · ", "Offline · ") + haceCuanto((long) visto).toLowerCase(Locale.ROOT) : T.t("Desconectado", "Offline");
        }
        String pack = texto(a, "pack");
        if (booleano(a, "mundo")) return T.t("Jugando ", "Playing ") + (pack != null ? pack + " · " : "") + T.t("mundo abierto", "world open");
        String hostea = texto(a, "hostea");
        String servidor = texto(a, "servidor");
        if (booleano(a, "jugando") && servidor != null) return T.t("Jugando ", "Playing ") + (pack != null ? pack : "") + T.t(" · en ", " · on ") + servidor;
        if (booleano(a, "jugando")) return T.t("Jugando", "Playing") + (pack != null ? " " + pack : "");
        if (hostea != null) return T.t("En línea · hosteando ", "Online · hosting ") + hostea;
        return T.t("En línea", "Online");
    }

    private void menuAmigo(final JsonObject a) {
        final String uid = texto(a, "uid");
        final String nombre = texto(a, "nombre");
        final boolean favorito = booleano(a, "favorito");
        abrirMenu();
        opcion("chat", T.t("Enviar mensaje", "Send message"), false, () -> mensajeA(uid));
        opcion("star", favorito ? T.t("Quitar de favoritos", "Remove from favorites") : T.t("Agregar a favoritos", "Add to favorites"), false, () -> {
            JsonObject datos = new JsonObject();
            datos.addProperty("uid", uid);
            datos.addProperty("activo", !favorito);
            juego.puente("favorito_amigo", datos, (r, error) -> {
                if (error != null) juego.aviso(T.t("No se pudo", "Could not do it"), error);
            });
        });
        if (booleano(a, "unirse")) opcion("play", T.t("Unirse", "Join"), false, () -> unirse(uid, null));
        if (booleano(juego.estado, "puedoInvitar") && booleano(a, "enLinea")) opcion("send", T.t("Invitar a tu partida", "Invite to your game"), false, () -> invitar(uid));
        opcion("trash", T.t("Eliminar amigo", "Remove friend"), true, () -> confirmar(T.t("Eliminar a " + nombre, "Remove " + nombre),
            T.t("Dejarán de verse en sus listas de amigos.", "You will no longer see each other in your friend lists."), T.t("Eliminar", "Remove"), true,
            () -> accionAmigo("quitar_amigo", uid, null)));
    }

    // ---------- Mensajes

    private List<JsonObject> chats() {
        List<JsonObject> salida = new ArrayList<JsonObject>();
        for (JsonElement e : lista(juego.estado, "chats")) salida.add(e.getAsJsonObject());
        return salida;
    }

    private JsonObject chatActual() {
        if (chatAbierto == null) return null;
        for (JsonObject c : chats()) if (chatAbierto.equals(texto(c, "id"))) return c;
        return null;
    }

    private void pintarMensajes(Lienzo l, boolean cursorVisible) {
        double lx = LATERAL, lw = LISTA;
        l.rect(lx + lw, 0, 1, H, 0, Lienzo.BORDE);
        l.texto(T.t("Mensajes", "Messages"), lx + 18, 18, 19, true, Lienzo.TEXTO);
        botonIcono(l, "users", lx + lw - 42, 16, 30, SUAVE, this::nuevoGrupo);
        botonIcono(l, "user-plus", lx + lw - 76, 16, 30, SUAVE, this::agregarAmigoDialogo);

        // Conversaciones: favoritas, recientes y amigos con los que todavía no hay una.
        double arriba = 60, abajo = H - 8;
        recortar(l, lx, arriba, lw, abajo - arriba);
        double ty = arriba - scrollChats;
        List<JsonObject> favoritos = new ArrayList<JsonObject>(), recientes = new ArrayList<JsonObject>();
        Set<String> conChat = new HashSet<String>();
        for (JsonObject c : chats()) {
            if (booleano(c, "favorito")) favoritos.add(c);
            else recientes.add(c);
            if (!esGrupo(c) && texto(c, "amigo") != null) conChat.add(texto(c, "amigo"));
        }
        if (!favoritos.isEmpty()) {
            etiqueta(l, T.t("Favoritos", "Favorites"), lx + 18, ty + 2);
            ty += 22;
            for (JsonObject c : favoritos) ty = filaChat(l, c, lx, ty, lw);
            if (!recientes.isEmpty()) {
                etiqueta(l, T.t("Recientes", "Recent"), lx + 18, ty + 8);
                ty += 28;
            }
        }
        for (JsonObject c : recientes) ty = filaChat(l, c, lx, ty, lw);
        boolean titulo = false;
        for (JsonElement e : lista(juego.estado, "amigos")) {
            final JsonObject a = e.getAsJsonObject();
            final String uid = texto(a, "uid");
            if (uid == null || conChat.contains(uid)) continue;
            if (!titulo) {
                titulo = true;
                etiqueta(l, T.t("Escribir a un amigo", "Write to a friend"), lx + 18, ty + 8);
                ty += 28;
            }
            boolean sobre = zona(lx + 8, ty, lw - 16, 46, () -> mensajeA(uid), null);
            if (sobre) l.rect(lx + 8, ty, lw - 16, 46, 9, FILA);
            cabezaAmigo(l, texto(a, "nombre"), a, lx + 16, ty + 7, 32, sobre ? FILA : Lienzo.FONDO);
            l.texto(l.recortar(texto(a, "nombre"), lw - 80, 13, true), lx + 58, ty + 6, 13, true, Lienzo.TEXTO);
            l.texto(l.recortar(estadoAmigo(a), lw - 80, 11, false), lx + 58, ty + 24, 11, false, Lienzo.APAGADO);
            ty += 48;
        }
        if (chats().isEmpty() && !titulo) {
            for (String linea : l.lineas(T.t("Todavía no tienes conversaciones. Agrega amigos con el botón de arriba.", "You have no conversations yet. Add friends with the button above."), lw - 36, 12, false)) {
                l.texto(linea, lx + 18, ty + 4, 12, false, Lienzo.APAGADO);
                ty += 17;
            }
        }
        maxChats = Math.max(0, ty + scrollChats - abajo + 8);
        scrollChats = limitar(scrollChats, maxChats);
        sinRecorte(l);

        double cx = lx + lw, cw = W - cx;
        JsonObject chat = chatActual();
        if (chatAbierto == null || chat == null) {
            vacio(l, cx + cw / 2, H / 2, "chat", T.t("Elige una conversación", "Pick a conversation"), T.t("O escríbele a un amigo de la lista.", "Or write to a friend from the list."), null);
            return;
        }
        pintarConversacion(l, chat, cx, cw, cursorVisible);
    }

    private double filaChat(Lienzo l, final JsonObject c, double lx, double ty, double lw) {
        final String id = texto(c, "id");
        boolean abierto = id.equals(chatAbierto);
        boolean sobre = zona(lx + 8, ty, lw - 16, 50, () -> abrirChat(id), () -> menuChat(c));
        Color fondo = abierto ? Lienzo.ACENTO_SUAVE : sobre ? FILA : Lienzo.FONDO;
        if (abierto || sobre) l.rect(lx + 8, ty, lw - 16, 50, 9, fondo);
        avatarChat(l, c, lx + 16, ty + 8, 34, abierto ? new Color(0x2b, 0x22, 0x44) : sobre ? new Color(0x26, 0x24, 0x2d) : Lienzo.FONDO);
        boolean nuevo = booleano(c, "sinLeer");
        String hora = horaCorta((long) numero(c, "hora", 0));
        double hw = l.ancho(hora, 10.5, false);
        double nw = l.texto(l.recortar(texto(c, "nombre"), lw - 86 - hw, 13, true), lx + 58, ty + 7, 13, true, Lienzo.TEXTO);
        if (booleano(c, "favorito")) Iconos.dibujar(l, "star", lx + 58 + nw + 4, ty + 9, 11, new Color(0xe8a33d));
        l.texto(hora, lx + lw - 18 - hw, ty + 9, 10.5, false, Lienzo.TENUE);
        String ultimo = vistaPrevia(texto(c, "ultimo"));
        if (esGrupo(c) && texto(c, "ultimoNombre") != null && !ultimo.isEmpty()) ultimo = texto(c, "ultimoNombre") + ": " + ultimo;
        if (ultimo.isEmpty()) ultimo = esGrupo(c) ? T.t("Grupo nuevo", "New group") : T.t("Sin mensajes todavía", "No messages yet");
        l.texto(l.recortar(ultimo, lw - 92, 11.5, nuevo), lx + 58, ty + 26, 11.5, nuevo, nuevo ? Lienzo.TEXTO : Lienzo.APAGADO);
        if (nuevo) l.circulo(lx + lw - 27, ty + 29, 9, Lienzo.ACENTO);
        return ty + 52;
    }

    private void pintarConversacion(Lienzo l, final JsonObject chat, double cx, double cw, boolean cursorVisible) {
        final boolean grupo = esGrupo(chat);
        final JsonObject amigo = grupo ? null : amigosPorUid.get(texto(chat, "amigo"));
        // Cabecera
        if (grupo) {
            boolean sobreAvatar = zona(cx + 16, 12, 34, 34, () -> miembros(chat), null);
            avatarGrupo(l, chat, cx + 16, 12, 34);
            if (sobreAvatar) l.borde(cx + 16, 12, 34, 34, 9, Lienzo.ACENTO_TEXTO);
        } else {
            cabezaAmigo(l, texto(chat, "nombre"), amigo, cx + 16, 12, 34, Lienzo.FONDO);
        }
        double derecha = W - 14;
        botonIcono(l, "more", derecha - 30, 14, 30, SUAVE, () -> menuChat(chat));
        derecha -= 34;
        final boolean favorito = booleano(chat, "favorito");
        botonIcono(l, "star", derecha - 30, 14, 30, SUAVE, () -> favoritoChat(chat, !favorito));
        if (favorito) Iconos.dibujar(l, "star", derecha - 30 + 8, 22, 14, new Color(0xe8a33d));
        derecha -= 36;
        if (amigo != null && booleano(amigo, "unirse")) {
            final String uid = texto(amigo, "uid");
            derecha = botonDerecha(l, T.t("Unirse", "Join"), "play", derecha, 15, 28, PRIMARIO, () -> unirse(uid, null)) - 6;
        }
        double textoW = derecha - cx - 62;
        l.texto(l.recortar(texto(chat, "nombre"), textoW, 15, true), cx + 60, 11, 15, true, Lienzo.TEXTO);
        String sub;
        if (grupo) {
            StringBuilder nombres = new StringBuilder();
            for (JsonElement m : lista(chat, "miembros")) {
                if (nombres.length() > 0) nombres.append(", ");
                nombres.append(texto(m.getAsJsonObject(), "nombre"));
            }
            sub = nombres.toString();
        } else {
            sub = amigo != null ? estadoAmigo(amigo) : "";
        }
        l.texto(l.recortar(sub, textoW, 11.5, false), cx + 60, 31, 11.5, false, Lienzo.APAGADO);
        l.rect(cx, 58, cw, 1, 0, Lienzo.BORDE);

        // Escribir (con la respuesta encima)
        double campoY = H - 50;
        double areaAbajo = campoY - 8;
        if (respondiendo != null) {
            double ry = campoY - 46;
            l.rect(cx + 14, ry, cw - 28, 40, 8, Lienzo.TARJETA);
            l.rect(cx + 14, ry, 3, 40, 1.5, Lienzo.ACENTO);
            Iconos.dibujar(l, "reply", cx + 26, ry + 12, 15, Lienzo.ACENTO_TEXTO);
            String quien = booleano(respondiendo, "mine") ? T.t("Respondiendo a tu mensaje", "Replying to your message") : T.t("Respondiendo a ", "Replying to ") + texto(respondiendo, "name");
            l.texto(l.recortar(quien, cw - 120, 11.5, true), cx + 50, ry + 5, 11.5, true, Lienzo.ACENTO_TEXTO);
            l.texto(l.recortar(texto(respondiendo, "text"), cw - 120, 11.5, false), cx + 50, ry + 21, 11.5, false, Lienzo.APAGADO);
            botonIcono(l, "close", cx + cw - 46, ry + 7, 26, SUAVE, () -> respondiendo = null);
            areaAbajo = ry - 6;
        } else if (editando != null) {
            l.texto(T.t("Editando · Esc para cancelar", "Editing · Esc to cancel"), cx + 18, campoY - 18, 11, false, Lienzo.ACENTO_TEXTO);
            areaAbajo = campoY - 22;
        }
        botonIcono(l, editando != null ? "check" : "send", W - 14 - 38, campoY, 38, PRIMARIO, this::enviarMensaje);
        zona(cx + 14, campoY, cw - 28 - 46, 38, () -> escribir.foco = true, null);
        escribir.dibujar(l, cx + 14, campoY, cw - 28 - 46, 38, cursorVisible && escribir.foco);

        // Mensajes
        double areaArriba = 60;
        recortar(l, cx, areaArriba, cw, areaAbajo - areaArriba);
        if (mensajes == null) {
            l.texto(T.t("Cargando…", "Loading…"), cx + 20, areaArriba + 16, 13, false, Lienzo.APAGADO);
        } else if (mensajes.size() == 0) {
            String t = T.t("Todavía no hay mensajes. ¡Saluda!", "No messages yet. Say hi!");
            l.texto(t, cx + (cw - l.ancho(t, 13, false)) / 2, (areaArriba + areaAbajo) / 2 - 8, 13, false, Lienzo.APAGADO);
        } else {
            pintarBurbujas(l, cx, cw, areaArriba, areaAbajo, grupo);
        }
        sinRecorte(l);
    }

    /** Una fila de mensaje ya medida. */
    private static final class Fila {
        JsonObject m;
        String dia;
        boolean continua, autor;
        double alto, burbujaW, burbujaH, citaH, textoH, reaccionesH;
        List<String> lineas;
    }

    private String filaEncima;
    private final List<Object[]> filasPintadas = new ArrayList<Object[]>();

    /** El mensaje bajo el ratón (para mostrar sus herramientas). */
    private String filaBajo() {
        for (Object[] f : filasPintadas) {
            double fy = (Double) f[1], fh = (Double) f[2];
            if (ratonY >= fy && ratonY < fy + fh && ratonX > LATERAL + LISTA) return (String) f[0];
        }
        return null;
    }

    private void pintarBurbujas(Lienzo l, double cx, double cw, double arriba, double abajo, boolean grupo) {
        double maxBurbuja = Math.min(330, cw * 0.70);
        List<Fila> filas = new ArrayList<Fila>();
        double total = 0;
        String anterior = null;
        long antes = 0;
        String diaAnterior = null;
        for (JsonElement e : mensajes) {
            Fila f = new Fila();
            f.m = e.getAsJsonObject();
            long at = (long) numero(f.m, "at", 0);
            String dia = diaDe(at);
            f.dia = dia.equals(diaAnterior) ? null : dia;
            diaAnterior = dia;
            boolean mio = booleano(f.m, "mine");
            f.continua = f.dia == null && texto(f.m, "from") != null && texto(f.m, "from").equals(anterior) && at - antes < 5 * 60000;
            f.autor = grupo && !mio && !f.continua;
            anterior = texto(f.m, "from");
            antes = at;
            medir(l, f, maxBurbuja);
            filas.add(f);
            total += f.alto;
        }
        double visible = abajo - arriba;
        maxMensajes = Math.max(0, total - visible + 12);
        scrollMensajes = limitar(scrollMensajes, maxMensajes);
        double ty = abajo - total - 6 + scrollMensajes;
        if (total < visible) ty = abajo - total - 6;
        filasPintadas.clear();
        String bajo = null;
        for (Fila f : filas) {
            double fy = ty + (f.dia != null ? 30 : 0) + (f.continua ? 3 : 9);
            double fh = f.alto - (f.dia != null ? 30 : 0) - (f.continua ? 3 : 9);
            filasPintadas.add(new Object[] {texto(f.m, "id"), fy, fh});
            if (ratonY >= fy && ratonY < fy + fh && ratonX > cx && ratonX < cx + cw && ratonY >= arriba && ratonY < abajo) bajo = texto(f.m, "id");
            ty += f.alto;
        }
        filaEncima = debajo || menu != null || menuEmojis != null ? null : bajo;
        ty = abajo - total - 6 + scrollMensajes;
        if (total < visible) ty = abajo - total - 6;
        for (Fila f : filas) {
            if (ty + f.alto >= arriba && ty <= abajo) pintarFila(l, f, cx, cw, ty, maxBurbuja);
            ty += f.alto;
        }
    }

    private static boolean tarjeta(String texto) {
        return texto != null && (texto.startsWith(INVITACION) || texto.startsWith(COMPARTIR) || texto.startsWith(COMPARTIR2));
    }

    private void medir(Lienzo l, Fila f, double maximo) {
        JsonObject m = f.m;
        String texto = texto(m, "text");
        JsonObject reply = objeto(m, "reply");
        f.citaH = reply != null ? 40 : 0;
        if (booleano(m, "image")) {
            f.burbujaW = Math.min(240, maximo - 16) + 12;
            f.burbujaH = altoFoto(m, maximo) + 22 + f.citaH;
        } else if (tarjeta(texto)) {
            f.burbujaW = 280;
            f.burbujaH = 64;
        } else {
            f.lineas = l.lineas(texto, maximo - 24, 13, false);
            double ancho = l.ancho(hora(m), 10, false) + 4;
            for (String linea : f.lineas) ancho = Math.max(ancho, l.ancho(linea, 13, false));
            if (reply != null) ancho = Math.max(ancho, Math.min(maximo - 24, Math.max(l.ancho(texto(reply, "name"), 11, true), l.ancho(vistaPrevia(texto(reply, "text")), 11.5, false))));
            if (f.autor) ancho = Math.max(ancho, l.ancho(texto(m, "name"), 11.5, true));
            f.textoH = f.lineas.size() * l.altoLinea(13);
            f.burbujaW = Math.min(maximo, ancho + 24);
            f.burbujaH = f.textoH + 26 + f.citaH + (f.autor ? 17 : 0);
        }
        JsonArray reacciones = lista(m, "reactions");
        f.reaccionesH = reacciones.size() > 0 ? 22 : 0;
        f.alto = (f.dia != null ? 30 : 0) + (f.continua ? 3 : 9) + f.burbujaH + (f.reaccionesH > 0 ? f.reaccionesH - 2 : 0);
    }

    private static String hora(JsonObject m) {
        String hora = new SimpleDateFormat("HH:mm").format(new Date((long) numero(m, "at", 0)));
        return booleano(m, "edited") ? T.t("editado · ", "edited · ") + hora : hora;
    }

    private void pintarFila(Lienzo l, final Fila f, double cx, double cw, double ty, double maximo) {
        final JsonObject m = f.m;
        final boolean mio = booleano(m, "mine");
        final String id = texto(m, "id");
        if (f.dia != null) {
            double dw = l.ancho(f.dia, 11, true) + 22;
            l.rect(cx + (cw - dw) / 2, ty + 6, dw, 20, 10, Lienzo.TARJETA);
            l.texto(f.dia, cx + (cw - dw) / 2 + 11, ty + 9, 11, true, Lienzo.APAGADO);
            ty += 30;
        }
        ty += f.continua ? 3 : 9;
        double bx = mio ? cx + cw - 18 - f.burbujaW : cx + 18;
        // Toda la fila: clic derecho para el menú del mensaje.
        zona(cx, ty - (f.continua ? 3 : 9), cw, f.burbujaH + (f.continua ? 3 : 9) + f.reaccionesH, null, () -> menuMensaje(m));
        String texto = texto(m, "text");
        Color fondo = mio ? Lienzo.ACENTO : Lienzo.TARJETA;
        Color letra = mio ? Color.WHITE : Lienzo.TEXTO;
        Color tenue = mio ? new Color(255, 255, 255, 190) : Lienzo.TENUE;
        if (tarjeta(texto)) {
            pintarTarjeta(l, m, texto, bx, ty, mio);
        } else {
            l.rect(bx, ty, f.burbujaW, f.burbujaH, 12, fondo);
            double iy = ty + 7;
            if (f.autor) {
                l.texto(l.recortar(texto(m, "name"), f.burbujaW - 24, 11.5, true), bx + 12, iy, 11.5, true, Lienzo.ACENTO_TEXTO);
                iy += 17;
            }
            JsonObject reply = objeto(m, "reply");
            if (reply != null) pintarCita(l, reply, bx + 7, iy - 1, f.burbujaW - 14, mio);
            iy += f.citaH;
            if (booleano(m, "image")) {
                double fotoAlto = altoFoto(m, maximo), fotoAncho = f.burbujaW - 12;
                BufferedImage foto = foto(id);
                if (foto != null) l.imagen(foto, bx + 6, iy - 1, fotoAncho, fotoAlto, 8);
                else l.rect(bx + 6, iy - 1, fotoAncho, fotoAlto, 8, new Color(0, 0, 0, 60));
                String hora = hora(m);
                l.texto(hora, bx + f.burbujaW - 8 - l.ancho(hora, 10, false), iy + fotoAlto + 1, 10, false, tenue);
            } else {
                for (String linea : f.lineas) {
                    l.texto(linea, bx + 12, iy, 13, false, letra);
                    iy += l.altoLinea(13);
                }
                String hora = hora(m);
                l.texto(hora, bx + f.burbujaW - 12 - l.ancho(hora, 10, false), iy + 1, 10, false, tenue);
            }
        }
        // Reacciones debajo de la burbuja
        JsonArray reacciones = lista(m, "reactions");
        if (reacciones.size() > 0) {
            double rx = mio ? bx + f.burbujaW - 8 : bx + 8;
            double ry = ty + f.burbujaH - 4;
            List<Object[]> chips = new ArrayList<Object[]>();
            double totalW = 0;
            for (JsonElement e : reacciones) {
                JsonObject r = e.getAsJsonObject();
                int cuenta = (int) numero(r, "count", 1);
                double w = 16 + 12 + (cuenta > 1 ? l.ancho(String.valueOf(cuenta), 11, true) + 4 : 0);
                chips.add(new Object[] {r, w});
                totalW += w + 4;
            }
            double chipX = mio ? rx - totalW + 4 : rx;
            for (Object[] c : chips) {
                final JsonObject r = (JsonObject) c[0];
                double w = (Double) c[1];
                final String emoji = texto(r, "emoji");
                boolean mia = booleano(r, "mine");
                boolean sobre = zona(chipX, ry, w, 22, () -> reaccionar(m, emoji), null);
                l.rect(chipX, ry, w, 22, 11, mia ? new Color(0x3a, 0x2c, 0x63) : (sobre ? Lienzo.TARJETA_ENCIMA : new Color(0x26, 0x24, 0x2d)));
                l.borde(chipX, ry, w, 22, 11, mia ? Lienzo.ACENTO : Lienzo.BORDE);
                Emojis.dibujar(l, emoji, chipX + 7, ry + 3, 16);
                int cuenta = (int) numero(r, "count", 1);
                if (cuenta > 1) l.texto(String.valueOf(cuenta), chipX + 26, ry + 4, 11, true, mia ? Lienzo.ACENTO_TEXTO : Lienzo.APAGADO);
                if (sobre) {
                    // Quién reaccionó, encima del chip.
                    StringBuilder quienes = new StringBuilder();
                    for (JsonElement n : lista(r, "names")) {
                        if (quienes.length() > 0) quienes.append(", ");
                        quienes.append(n.getAsString());
                    }
                    String q = l.recortar(quienes.toString(), 220, 11, false);
                    double qw = l.ancho(q, 11, false) + 16;
                    double qx = Math.min(Math.max(cx + 4, chipX + w / 2 - qw / 2), cx + cw - qw - 4);
                    l.rect(qx, ry - 26, qw, 22, 7, Lienzo.TARJETA_ENCIMA);
                    l.texto(q, qx + 8, ry - 22, 11, false, Lienzo.TEXTO);
                }
                chipX += w + 4;
            }
        }
        // Herramientas al pasar el ratón: reaccionar, responder y, en los tuyos, editar y borrar.
        if (id != null && id.equals(filaEncima)) {
            List<Object[]> herramientas = new ArrayList<Object[]>();
            herramientas.add(new Object[] {"smile", (Accion) () -> selectorReacciones(m)});
            herramientas.add(new Object[] {"reply", (Accion) () -> responder(m)});
            if (mio && editable(m)) herramientas.add(new Object[] {"pencil", (Accion) () -> editar(m)});
            if (mio) herramientas.add(new Object[] {"trash", (Accion) () -> confirmarBorrar(id)});
            double d = 26, hw = herramientas.size() * (d + 2);
            double hx = mio ? bx - hw - 4 : bx + f.burbujaW + 4;
            double hy = ty + Math.max(0, (Math.min(f.burbujaH, 40) - d) / 2);
            l.rect(hx - 3, hy - 3, hw + 4, d + 6, 9, new Color(0x17, 0x16, 0x1b, 230));
            for (Object[] h : herramientas) {
                botonIcono(l, (String) h[0], hx, hy, d, SUAVE, (Accion) h[1]);
                hx += d + 2;
            }
        }
    }

    /** La cita del mensaje al que responde (o «Mensaje borrado» si ya no está entre los cargados). */
    private void pintarCita(Lienzo l, JsonObject reply, double qx, double qy, double qw, boolean mio) {
        final String original = texto(reply, "id");
        JsonObject encontrado = null;
        if (mensajes != null) for (JsonElement e : mensajes) if (original != null && original.equals(texto(e.getAsJsonObject(), "id"))) encontrado = e.getAsJsonObject();
        boolean borrado = encontrado == null && mensajes != null && mensajes.size() > 0 && original != null && original.compareTo(String.valueOf(texto(mensajes.get(0).getAsJsonObject(), "id"))) >= 0;
        String nombre = encontrado != null ? texto(encontrado, "name") : texto(reply, "name");
        String texto = borrado ? T.t("Mensaje borrado", "Deleted message") : encontrado != null ? previa(encontrado) : vistaPrevia(texto(reply, "text"));
        boolean sobre = zona(qx, qy, qw, 36, () -> irAMensaje(original), null);
        l.rect(qx, qy, qw, 36, 6, sobre ? new Color(0, 0, 0, 70) : CITA);
        l.rect(qx, qy, 3, 36, 1.5, mio ? new Color(255, 255, 255, 180) : Lienzo.ACENTO);
        l.texto(l.recortar(nombre, qw - 20, 11, true), qx + 10, qy + 3, 11, true, mio ? Color.WHITE : Lienzo.ACENTO_TEXTO);
        l.texto(l.recortar(texto, qw - 20, 11.5, false), qx + 10, qy + 18, 11.5, false, mio ? new Color(255, 255, 255, 210) : Lienzo.APAGADO);
    }

    /** Invitación (con «Unirse») o instancia compartida (se abre en Parabellum). */
    @SuppressWarnings("deprecation")
    private void pintarTarjeta(Lienzo l, JsonObject m, String texto, double bx, double ty, boolean mio) {
        double w = 280, h = 64;
        l.rect(bx, ty, w, h, 12, mio ? new Color(0x5e, 0x3a, 0xc0) : Lienzo.TARJETA);
        l.borde(bx, ty, w, h, 12, Lienzo.ACENTO);
        if (texto.startsWith(INVITACION)) {
            JsonObject datos;
            try {
                datos = new JsonParser().parse(texto.substring(INVITACION.length())).getAsJsonObject();
            } catch (Exception e) {
                datos = new JsonObject();
            }
            boolean servidor = "servidor".equals(texto(datos, "t"));
            l.texto(mio ? T.t("Invitaste", "You invited") : servidor ? T.t("Te invitó a su servidor", "Invited you to their server") : T.t("Te invitó a su mundo", "Invited you to their world"), bx + 14, ty + 10, 11, true, Lienzo.ACENTO_TEXTO);
            l.texto(l.recortar(texto(datos, "n"), w - 120, 14, true), bx + 14, ty + 28, 14, true, Color.WHITE);
            if (!mio) {
                final String anfitrion = texto(m, "from");
                final String clave = servidor ? texto(datos, "k") : null;
                botonDerecha(l, T.t("Unirse", "Join"), "play", bx + w - 12, ty + 16, 32, PRIMARIO, () -> unirse(anfitrion, clave));
            }
        } else {
            l.texto(T.t("Instancia compartida", "Shared instance"), bx + 14, ty + 10, 11, true, Lienzo.ACENTO_TEXTO);
            l.texto(T.t("Ábrela en Parabellum", "Open it in Parabellum"), bx + 14, ty + 28, 14, true, Color.WHITE);
            botonDerecha(l, T.t("Abrir", "Open"), null, bx + w - 12, ty + 16, 32, NORMAL, () -> juego.puente("abrir_parabellum", null, null));
        }
    }

    private static boolean editable(JsonObject m) {
        String texto = texto(m, "text");
        return booleano(m, "mine") && !booleano(m, "image") && texto != null && !tarjeta(texto);
    }

    private void menuMensaje(final JsonObject m) {
        final String id = texto(m, "id");
        abrirMenu();
        emojisDelMenu(m);
        opcion("reply", T.t("Responder", "Reply"), false, () -> responder(m));
        final String texto = texto(m, "text");
        if (texto != null && !tarjeta(texto)) opcion("copy", T.t("Copiar texto", "Copy text"), false, () -> {
            juego.plataforma.copiar(texto);
            juego.aviso(T.t("Texto copiado", "Text copied"), null);
        });
        if (editable(m)) opcion("pencil", T.t("Editar", "Edit"), false, () -> editar(m));
        if (booleano(m, "mine")) opcion("trash", T.t("Borrar", "Delete"), true, () -> confirmarBorrar(id));
    }

    private void selectorReacciones(JsonObject m) {
        menu = null;
        emojisDelMenu(m);
        menuX = ratonX - 110;
        menuY = ratonY - 52;
    }

    private void emojisDelMenu(final JsonObject m) {
        String mia = null;
        for (JsonElement e : lista(m, "reactions")) if (booleano(e.getAsJsonObject(), "mine")) mia = texto(e.getAsJsonObject(), "emoji");
        menuEmojis = new String[] {mia};
        menuEmojiAccion = new Accion[Emojis.REACCIONES.length];
        for (int i = 0; i < Emojis.REACCIONES.length; i++) {
            final String emoji = Emojis.REACCIONES[i];
            menuEmojiAccion[i] = () -> reaccionar(m, emoji);
        }
    }

    private void menuChat(final JsonObject chat) {
        final boolean grupo = esGrupo(chat);
        final boolean favorito = booleano(chat, "favorito");
        abrirMenu();
        opcion("star", favorito ? T.t("Quitar de favoritos", "Remove from favorites") : T.t("Agregar a favoritos", "Add to favorites"), false, () -> favoritoChat(chat, !favorito));
        if (grupo) {
            opcion("users", T.t("Miembros", "Members"), false, () -> miembros(chat));
            if (booleano(chat, "puedoEditar")) {
                opcion("user-plus", T.t("Agregar amigos", "Add friends"), false, () -> agregarAlGrupo(chat));
                opcion("pencil", T.t("Cambiar nombre", "Rename"), false, () -> renombrar(chat));
            }
            final boolean dueno = soyDueno(chat);
            opcion("logout", T.t("Salir del grupo", "Leave the group"), true, () -> confirmar(T.t("Salir del grupo", "Leave the group"),
                dueno ? T.t("Dejarás de ver sus mensajes y el grupo pasará a otro miembro.", "You will stop seeing its messages and the group will pass to another member.")
                    : T.t("Dejarás de ver sus mensajes. Quien creó el grupo puede volver a agregarte.", "You will stop seeing its messages. Whoever created the group can add you again."),
                T.t("Salir", "Leave"), true, () -> salir(chat)));
        } else {
            opcion("close", T.t("Ocultar conversación", "Hide conversation"), true, () -> confirmar(T.t("Ocultar conversación", "Hide conversation"),
                T.t("Desaparece de tu lista; vuelve si te escribe de nuevo.", "It disappears from your list; it comes back if they write to you again."), T.t("Ocultar", "Hide"), true, () -> salir(chat)));
        }
    }

    // ---------- Menú (clic derecho y ⋯), con una fila de reacciones arriba si es de un mensaje

    private void abrirMenu() {
        menu = new ArrayList<Object[]>();
        menuEmojis = null;
        menuX = ratonX;
        menuY = ratonY;
    }

    private void opcion(String icono, String texto, boolean peligro, Accion accion) {
        menu.add(new Object[] {icono, texto, peligro, accion});
    }

    private void cerrarMenu() {
        menu = null;
        menuEmojis = null;
        menuEmojiAccion = null;
    }

    private void pintarMenu(Lienzo l) {
        double w = Math.max(menu != null ? 200 : 0, menuEmojis != null ? Emojis.REACCIONES.length * 34 + 8 : 0);
        double filaEmojis = menuEmojis != null ? 44 : 0;
        double h = (menu != null ? menu.size() * 32 : 0) + filaEmojis + 8 + (menu != null && menuEmojis != null ? 6 : 0);
        double mx = Math.max(8, Math.min(menuX, W - w - 8)), my = Math.max(8, Math.min(menuY, H - h - 8));
        l.rect(mx, my, w, h, 10, Lienzo.TARJETA);
        l.borde(mx, my, w, h, 10, Lienzo.BORDE);
        List<Object[]> opciones = menu;
        String[] emojis = menuEmojis;
        Accion[] acciones = menuEmojiAccion;
        registrandoMenu = true;
        double oy = my + 4;
        if (emojis != null) {
            double ex = mx + 4;
            for (int i = 0; i < Emojis.REACCIONES.length; i++) {
                String emoji = Emojis.REACCIONES[i];
                boolean actual = emoji.equals(emojis[0]);
                boolean sobre = zona(ex, oy, 34, 36, acciones[i], null);
                if (actual || sobre) l.rect(ex, oy, 34, 36, 8, actual ? Lienzo.ACENTO_SUAVE : Lienzo.TARJETA_ENCIMA);
                double t = sobre ? 24 : 21;
                Emojis.dibujar(l, emoji, ex + (34 - t) / 2, oy + (36 - t) / 2, t);
                ex += 34;
            }
            oy += filaEmojis;
            if (opciones != null) {
                l.rect(mx + 8, oy - 3, w - 16, 1, 0, Lienzo.BORDE);
                oy += 6;
            }
        }
        if (opciones != null) {
            for (Object[] o : opciones) {
                final Accion accion = (Accion) o[3];
                boolean sobre = zona(mx + 4, oy, w - 8, 32, accion, null);
                if (sobre) l.rect(mx + 4, oy, w - 8, 32, 7, Lienzo.TARJETA_ENCIMA);
                boolean peligro = (Boolean) o[2];
                Color color = peligro ? Lienzo.PELIGRO : Lienzo.TEXTO;
                Iconos.dibujar(l, (String) o[0], mx + 13, oy + 8, 16, peligro ? Lienzo.PELIGRO : Lienzo.APAGADO);
                l.texto((String) o[1], mx + 38, oy + 8, 13, false, color);
                oy += 32;
            }
        }
        registrandoMenu = false;
    }

    // ---------- Diálogos

    private static boolean conCampo(Dialogo d) {
        return "grupo_nuevo".equals(d.tipo) || "renombrar".equals(d.tipo) || "agregar_amigo".equals(d.tipo);
    }

    private void abrirDialogo(Dialogo d) {
        cerrarMenu();
        dialogo = d;
        scrollDialogo = 0;
        agregar.foco = false;
        buscar.foco = false;
        escribir.foco = false;
        campoDialogo.foco = conCampo(d);
    }

    private void cerrarDialogo() {
        dialogo = null;
        campoDialogo.foco = false;
        campoDialogo.poner("");
    }

    private void confirmar(String titulo, String texto, String ok, boolean peligro, Accion accion) {
        Dialogo d = new Dialogo("confirmar");
        d.titulo = titulo;
        d.texto = texto;
        d.ok = ok;
        d.peligro = peligro;
        d.accion = accion;
        abrirDialogo(d);
    }

    private void nuevoGrupo() {
        Dialogo d = new Dialogo("grupo_nuevo");
        d.titulo = T.t("Nuevo grupo", "New group");
        d.ok = T.t("Crear grupo", "Create group");
        campoDialogo.poner("");
        abrirDialogo(d);
    }

    private void agregarAlGrupo(JsonObject chat) {
        Dialogo d = new Dialogo("agregar_grupo");
        d.titulo = T.t("Agregar amigos al grupo", "Add friends to the group");
        d.ok = T.t("Agregar", "Add");
        d.chat = texto(chat, "id");
        abrirDialogo(d);
    }

    private void renombrar(JsonObject chat) {
        Dialogo d = new Dialogo("renombrar");
        d.titulo = T.t("Cambiar nombre del grupo", "Rename the group");
        d.ok = T.t("Guardar", "Save");
        d.chat = texto(chat, "id");
        campoDialogo.poner(texto(chat, "nombre"));
        abrirDialogo(d);
    }

    private void miembros(JsonObject chat) {
        Dialogo d = new Dialogo("miembros");
        d.titulo = T.t("Miembros del grupo", "Group members");
        d.ok = T.t("Listo", "Done");
        d.chat = texto(chat, "id");
        abrirDialogo(d);
    }

    private void agregarAmigoDialogo() {
        Dialogo d = new Dialogo("agregar_amigo");
        d.titulo = T.t("Agregar amigo", "Add friend");
        d.ok = T.t("Enviar solicitud", "Send request");
        campoDialogo.poner("");
        abrirDialogo(d);
    }

    private void pintarDialogo(Lienzo l, boolean cursorVisible) {
        final Dialogo d = dialogo;
        // Fondo: oscurece el panel; un clic fuera cierra.
        l.rect(0, 0, W, H, 14, new Color(8, 7, 11, 150));
        zona(0, 0, W, H, this::cerrarDialogo, null);
        boolean lista = "grupo_nuevo".equals(d.tipo) || "agregar_grupo".equals(d.tipo) || "miembros".equals(d.tipo);
        double dw = 400, dh = lista ? 420 : "confirmar".equals(d.tipo) ? 180 : 210;
        double dx = (W - dw) / 2, dy = (H - dh) / 2;
        l.rect(dx, dy, dw, dh, 14, Lienzo.FONDO);
        l.borde(dx, dy, dw, dh, 14, Lienzo.BORDE);
        zona(dx, dy, dw, dh, () -> { }, null);
        l.texto(l.recortar(d.titulo, dw - 40, 17, true), dx + 22, dy + 20, 17, true, Lienzo.TEXTO);
        double ty = dy + 54;
        if (d.texto != null) {
            for (String linea : l.lineas(d.texto, dw - 44, 13, false)) {
                l.texto(linea, dx + 22, ty, 13, false, Lienzo.APAGADO);
                ty += 19;
            }
            ty += 6;
        }
        if (conCampo(d)) {
            l.texto("agregar_amigo".equals(d.tipo) ? T.t("Su nombre de jugador en Parabellum", "Their player name in Parabellum") : T.t("Nombre del grupo", "Group name"), dx + 22, ty, 12, true, Lienzo.APAGADO);
            ty += 20;
            zona(dx + 22, ty, dw - 44, 36, () -> campoDialogo.foco = true, null);
            campoDialogo.dibujar(l, dx + 22, ty, dw - 44, 36, cursorVisible && campoDialogo.foco);
            ty += 46;
        }
        if (lista) {
            l.texto("miembros".equals(d.tipo) ? T.t("Miembros", "Members") : T.t("Amigos", "Friends"), dx + 22, ty, 12, true, Lienzo.APAGADO);
            ty += 20;
            double abajo = dy + dh - 64 - (d.nota != null || "miembros".equals(d.tipo) ? 18 : 0);
            recortar(l, dx + 14, ty, dw - 28, abajo - ty);
            double fy = ty - scrollDialogo;
            if ("miembros".equals(d.tipo)) fy = pintarMiembros(l, d, dx, fy, dw);
            else fy = pintarEleccion(l, d, dx, fy, dw);
            maxDialogo = Math.max(0, fy + scrollDialogo - abajo);
            scrollDialogo = limitar(scrollDialogo, maxDialogo);
            sinRecorte(l);
        }
        if (d.error != null || d.nota != null) {
            String t = d.error != null ? d.error : d.nota;
            l.texto(l.recortar(t, dw - 44, 12, false), dx + 22, dy + dh - 78, 12, false, d.error != null ? Lienzo.PELIGRO : Lienzo.ACENTO_TEXTO);
        }
        double by = dy + dh - 54;
        double bx = botonDerecha(l, d.ocupado ? T.t("Un momento…", "One moment…") : d.ok, null, dx + dw - 22, by, 34, d.peligro ? PELIGRO : PRIMARIO, d.ocupado ? null : this::aceptarDialogo);
        if (!"miembros".equals(d.tipo)) botonDerecha(l, T.t("Cancelar", "Cancel"), null, bx - 8, by, 34, NORMAL, this::cerrarDialogo);
    }

    /** Amigos para elegir (crear un grupo o agregar a uno). */
    private double pintarEleccion(Lienzo l, final Dialogo d, double dx, double fy, double dw) {
        Set<String> yaEstan = new HashSet<String>();
        if (d.chat != null) {
            for (JsonObject c : chats()) {
                if (!d.chat.equals(texto(c, "id"))) continue;
                for (JsonElement m : lista(c, "miembros")) yaEstan.add(texto(m.getAsJsonObject(), "uid"));
            }
        }
        boolean alguno = false;
        for (JsonElement e : lista(juego.estado, "amigos")) {
            final JsonObject a = e.getAsJsonObject();
            final String uid = texto(a, "uid");
            if (uid == null || yaEstan.contains(uid)) continue;
            alguno = true;
            final boolean elegido = d.elegidos.contains(uid);
            boolean sobre = zona(dx + 16, fy, dw - 32, 40, () -> {
                if (elegido) d.elegidos.remove(uid);
                else d.elegidos.add(uid);
                d.error = null;
            }, null);
            if (sobre) l.rect(dx + 16, fy, dw - 32, 40, 8, Lienzo.TARJETA);
            l.rect(dx + 26, fy + 11, 18, 18, 5, elegido ? Lienzo.ACENTO : Lienzo.CAMPO);
            l.borde(dx + 26, fy + 11, 18, 18, 5, elegido ? Lienzo.ACENTO : Lienzo.BORDE);
            if (elegido) Iconos.dibujar(l, "check", dx + 28, fy + 13, 14, Color.WHITE);
            cabezaAmigo(l, texto(a, "nombre"), a, dx + 54, fy + 7, 26, sobre ? Lienzo.TARJETA : Lienzo.FONDO);
            l.texto(l.recortar(texto(a, "nombre"), dw - 130, 13, true), dx + 90, fy + 11, 13, true, Lienzo.TEXTO);
            fy += 42;
        }
        if (!alguno) {
            l.texto(d.chat != null ? T.t("Todos tus amigos ya están en el grupo.", "All your friends are already in the group.") : T.t("Primero agrega amigos.", "Add friends first."), dx + 22, fy + 4, 12.5, false, Lienzo.APAGADO);
            fy += 24;
        }
        return fy;
    }

    /** Los miembros, con el dueño primero; el dueño puede sacar a alguien o pasarle el grupo. */
    private double pintarMiembros(Lienzo l, Dialogo d, double dx, double fy, double dw) {
        JsonObject chat = null;
        for (JsonObject c : chats()) if (d.chat != null && d.chat.equals(texto(c, "id"))) chat = c;
        if (chat == null) return fy;
        final JsonObject grupo = chat;
        final String dueno = texto(chat, "dueno");
        final boolean puedo = booleano(chat, "puedoEditar");
        List<JsonObject> miembros = new ArrayList<JsonObject>();
        for (JsonElement m : lista(chat, "miembros")) miembros.add(m.getAsJsonObject());
        java.util.Collections.sort(miembros, (a, b) -> {
            boolean da = texto(a, "uid").equals(dueno), db = texto(b, "uid").equals(dueno);
            if (da != db) return da ? -1 : 1;
            if (booleano(a, "tu") != booleano(b, "tu")) return booleano(a, "tu") ? -1 : 1;
            return String.valueOf(texto(a, "nombre")).compareToIgnoreCase(String.valueOf(texto(b, "nombre")));
        });
        String nombreDueno = null;
        for (final JsonObject m : miembros) {
            final String uid = texto(m, "uid");
            final String nombre = texto(m, "nombre");
            boolean esDueno = uid.equals(dueno);
            if (esDueno) nombreDueno = nombre;
            cabezaAmigo(l, nombre, amigosPorUid.get(uid), dx + 22, fy + 6, 28, Lienzo.FONDO);
            l.texto(l.recortar(nombre + (booleano(m, "tu") ? T.t(" (tú)", " (you)") : ""), dw - 190, 13, true), dx + 60, esDueno ? fy + 6 : fy + 12, 13, true, Lienzo.TEXTO);
            if (esDueno) {
                Iconos.dibujar(l, "crown", dx + 60, fy + 23, 12, new Color(0xe8a33d));
                l.texto(T.t("Creó el grupo", "Created the group"), dx + 76, fy + 22, 11, true, new Color(0xe8a33d));
            }
            if (puedo && !booleano(m, "tu")) {
                botonIcono(l, "close", dx + dw - 50, fy + 6, 28, SUAVE, () -> confirmar(T.t("Sacar a " + nombre, "Remove " + nombre),
                    T.t("Dejará de ver los mensajes del grupo. Puedes volver a agregarlo.", "They will stop seeing the group's messages. You can add them again."), T.t("Sacar", "Remove"), true,
                    () -> {
                        accionGrupo("sacar_del_grupo", grupo, uid);
                        miembros(grupo);
                    }));
                botonIcono(l, "crown", dx + dw - 82, fy + 6, 28, SUAVE, () -> confirmar(T.t("Pasarle el grupo a " + nombre, "Hand the group over to " + nombre),
                    T.t("Podrá cambiar el nombre, la foto y los miembros. Tú ya no.", "They will be able to change the name, the photo and the members. You will not."), T.t("Pasárselo", "Hand it over"), false,
                    () -> {
                        accionGrupo("pasar_grupo", grupo, uid);
                        miembros(grupo);
                    }));
            }
            fy += 42;
        }
        d.nota = !puedo && nombreDueno != null ? T.t("Solo " + nombreDueno + " puede cambiar el nombre, la foto y los miembros.", "Only " + nombreDueno + " can change the name, the photo and the members.") : null;
        return fy;
    }

    private void aceptarDialogo() {
        final Dialogo d = dialogo;
        if (d == null || d.ocupado) return;
        if ("confirmar".equals(d.tipo)) {
            cerrarDialogo();
            if (d.accion != null) d.accion.hacer();
        } else if ("miembros".equals(d.tipo)) {
            cerrarDialogo();
        } else if ("agregar_amigo".equals(d.tipo)) {
            final String nombre = campoDialogo.valor.trim();
            if (nombre.isEmpty()) {
                d.error = T.t("Escribe su nombre de jugador.", "Type their player name.");
                return;
            }
            d.ocupado = true;
            JsonObject datos = new JsonObject();
            datos.addProperty("nombre", nombre);
            juego.puente("solicitud", datos, (r, error) -> {
                d.ocupado = false;
                if (error != null) {
                    d.error = error;
                } else {
                    d.error = null;
                    d.nota = texto(r.getAsJsonObject(), "texto");
                    campoDialogo.poner("");
                }
                marcar();
            });
        } else if ("renombrar".equals(d.tipo)) {
            final String nombre = campoDialogo.valor.trim();
            if (nombre.isEmpty()) {
                d.error = T.t("Ponle un nombre al grupo.", "Give the group a name.");
                return;
            }
            JsonObject datos = new JsonObject();
            datos.addProperty("chat", d.chat);
            datos.addProperty("nombre", nombre);
            pedirDialogo(d, "editar_grupo", datos);
        } else if ("grupo_nuevo".equals(d.tipo) || "agregar_grupo".equals(d.tipo)) {
            boolean nuevo = "grupo_nuevo".equals(d.tipo);
            final String nombre = campoDialogo.valor.trim();
            if (nuevo && nombre.isEmpty()) {
                d.error = T.t("Ponle un nombre al grupo.", "Give the group a name.");
                campoDialogo.foco = true;
                return;
            }
            if (d.elegidos.isEmpty()) {
                d.error = T.t("Elige al menos un amigo.", "Choose at least one friend.");
                return;
            }
            JsonObject datos = new JsonObject();
            JsonArray amigos = new JsonArray();
            for (String uid : d.elegidos) amigos.add(new com.google.gson.JsonPrimitive(uid));
            datos.add("amigos", amigos);
            if (nuevo) datos.addProperty("nombre", nombre);
            else datos.addProperty("chat", d.chat);
            pedirDialogo(d, nuevo ? "crear_grupo" : "agregar_al_grupo", datos);
        }
    }

    /** Manda lo del diálogo a Parabellum: si sale bien se cierra (y si se creó un grupo, se abre). */
    private void pedirDialogo(final Dialogo d, final String op, JsonObject datos) {
        d.ocupado = true;
        d.error = null;
        juego.puente(op, datos, (r, error) -> {
            d.ocupado = false;
            if (error != null) {
                d.error = error;
            } else {
                if (dialogo == d) cerrarDialogo();
                if ("crear_grupo".equals(op) && r != null && r.isJsonObject()) irAChat(texto(r.getAsJsonObject(), "chat"));
            }
            marcar();
        });
    }

    private void accionGrupo(String op, JsonObject chat, String uid) {
        JsonObject datos = new JsonObject();
        datos.addProperty("chat", texto(chat, "id"));
        datos.addProperty("uid", uid);
        juego.puente(op, datos, (r, error) -> {
            if (error != null) juego.aviso(T.t("No se pudo", "Could not do it"), error);
        });
    }

    // ---------- Capturas

    private void pintarCapturas(Lienzo l) {
        titulo(l, T.t("Capturas", "Screenshots"), null);
        if (captura != null) {
            pintarEnviarCaptura(l);
            return;
        }
        String tecla = juego.plataforma.teclaCaptura();
        List<File> lista = juego.capturas.ultimas();
        if (!lista.isEmpty()) botonDerecha(l, T.t("Abrir carpeta", "Open folder"), "folder", W - 24, 16, 32, NORMAL, this::abrirCarpeta);
        String pista = T.t("Pulsa " + tecla + " en el juego para sacar una (en muchas laptops: Fn + " + tecla + "). Elige una para mandarla.",
            "Press " + tecla + " in the game to take one (on many laptops: Fn + " + tecla + "). Pick one to send it.");
        l.texto(l.recortar(pista, W - LATERAL - 48, 12, false), LATERAL + 24, 54, 12, false, Lienzo.APAGADO);
        if (lista.isEmpty()) {
            vacio(l, LATERAL + (W - LATERAL) / 2, H / 2 + 20, "camera", T.t("Todavía no hay capturas en este juego", "No screenshots in this game yet"),
                T.t("Se guardan en la carpeta screenshots de la instancia. Saca una con " + tecla + " (o Fn + " + tecla + ") y aparece aquí.",
                    "They are saved in the instance's screenshots folder. Take one with " + tecla + " (or Fn + " + tecla + ") and it shows up here."),
                new Object[] {T.t("Abrir carpeta", "Open folder"), (Accion) this::abrirCarpeta, "folder"});
            return;
        }
        double arriba = 80, abajo = H - 14;
        recortar(l, LATERAL, arriba, W - LATERAL, abajo - arriba);
        double gap = 12, columnas = 3;
        double cw = (W - LATERAL - 48 - gap * (columnas - 1)) / columnas, ch = cw * 9 / 16;
        double ty = arriba - scrollCapturas;
        for (int i = 0; i < lista.size(); i++) {
            final File f = lista.get(i);
            double cx = LATERAL + 24 + (i % 3) * (cw + gap);
            double cy = ty + (i / 3) * (ch + gap);
            boolean sobre = zona(cx, cy, cw, ch, () -> {
                captura = f;
                scrollEnviar = 0;
            }, null);
            BufferedImage mini = juego.capturas.miniatura(f, this::marcarLuego);
            if (mini != null) l.imagen(mini, cx, cy, cw, ch, 8);
            else l.rect(cx, cy, cw, ch, 8, Lienzo.TARJETA);
            String cuando = haceCuanto(f.lastModified());
            double ew = l.ancho(cuando, 11, true) + 14;
            l.rect(cx + 8, cy + ch - 28, ew, 20, 6, new Color(0x14, 0x13, 0x18, 200));
            l.texto(cuando, cx + 15, cy + ch - 25, 11, true, Lienzo.TEXTO);
            if (sobre) {
                l.borde(cx, cy, cw, ch, 8, Lienzo.ACENTO);
                String mandar = T.t("Mandar", "Send");
                double mw = l.ancho(mandar, 12, true) + 34;
                l.rect(cx + cw - mw - 8, cy + ch - 32, mw, 24, 7, Lienzo.ACENTO);
                Iconos.dibujar(l, "send", cx + cw - mw, cy + ch - 28, 14, Color.WHITE);
                l.texto(mandar, cx + cw - mw + 18, cy + ch - 28, 12, true, Color.WHITE);
            }
        }
        double filas = Math.ceil(lista.size() / columnas);
        maxCapturas = Math.max(0, filas * (ch + gap) - (abajo - arriba) + 10);
        sinRecorte(l);
    }

    private void abrirCarpeta() {
        File carpeta = new File(juego.plataforma.carpeta(), "screenshots");
        if (!carpeta.isDirectory()) carpeta.mkdirs();
        JsonObject datos = new JsonObject();
        datos.addProperty("ruta", carpeta.getAbsolutePath());
        juego.puente("abrir_carpeta", datos, (r, error) -> {
            if (error != null) juego.aviso(T.t("No se pudo abrir la carpeta", "Could not open the folder"), error);
            else juego.aviso(T.t("Carpeta de capturas abierta", "Screenshots folder opened"), T.t("Está en tu escritorio, fuera del juego.", "It is on your desktop, outside the game."));
        });
    }

    private void pintarEnviarCaptura(Lienzo l) {
        double px = LATERAL + 24, py = 58, pw = 340, ph = pw * 9 / 16;
        BufferedImage mini = juego.capturas.miniatura(captura, this::marcarLuego);
        if (mini != null) l.imagen(mini, px, py, pw, ph, 10);
        else l.rect(px, py, pw, ph, 10, Lienzo.TARJETA);
        l.texto(l.recortar(captura.getName(), pw, 11, false), px, py + ph + 8, 11, false, Lienzo.TENUE);
        boton(l, T.t("Volver", "Back"), "left", px, py + ph + 30, 32, NORMAL, () -> captura = null);
        double lx = px + pw + 24, lw = W - lx - 20;
        l.texto(enviando ? T.t("Enviando…", "Sending…") : T.t("Enviar a", "Send to"), lx, py, 13, true, Lienzo.APAGADO);
        double arriba = py + 24, abajo = H - 16;
        recortar(l, lx, arriba, lw, abajo - arriba);
        double ty = arriba - scrollEnviar;
        // Primero las conversaciones (chats y grupos); después los amigos con los que todavía no hay una.
        Set<String> conChat = new HashSet<String>();
        for (final JsonObject c : chats()) {
            final String id = texto(c, "id");
            final String nombre = texto(c, "nombre");
            if (!esGrupo(c) && texto(c, "amigo") != null) conChat.add(texto(c, "amigo"));
            boolean sobre = zona(lx, ty, lw, 42, enviando ? null : (Accion) () -> enviarCaptura(id, nombre), null);
            l.rect(lx, ty, lw, 42, 8, sobre && !enviando ? Lienzo.TARJETA_ENCIMA : Lienzo.TARJETA);
            avatarChat(l, c, lx + 8, ty + 7, 28, sobre && !enviando ? Lienzo.TARJETA_ENCIMA : Lienzo.TARJETA);
            l.texto(l.recortar(nombre, lw - 56, 13, true), lx + 46, ty + 12, 13, true, enviando ? Lienzo.APAGADO : Lienzo.TEXTO);
            ty += 48;
        }
        for (JsonElement e : lista(juego.estado, "amigos")) {
            final JsonObject a = e.getAsJsonObject();
            final String uid = texto(a, "uid");
            final String nombre = texto(a, "nombre");
            if (uid == null || conChat.contains(uid)) continue;
            boolean sobre = zona(lx, ty, lw, 42, enviando ? null : (Accion) () -> enviarCapturaAmigo(uid, nombre), null);
            l.rect(lx, ty, lw, 42, 8, sobre && !enviando ? Lienzo.TARJETA_ENCIMA : Lienzo.TARJETA);
            cabezaAmigo(l, nombre, a, lx + 8, ty + 7, 28, sobre && !enviando ? Lienzo.TARJETA_ENCIMA : Lienzo.TARJETA);
            l.texto(l.recortar(nombre, lw - 56, 13, true), lx + 46, ty + 12, 13, true, enviando ? Lienzo.APAGADO : Lienzo.TEXTO);
            ty += 48;
        }
        if (ty == arriba - scrollEnviar) l.texto(T.t("Agrega amigos para mandarles capturas.", "Add friends to send them screenshots."), lx, ty + 8, 12, false, Lienzo.APAGADO);
        maxEnviar = Math.max(0, ty + scrollEnviar - abajo + 6);
        scrollEnviar = limitar(scrollEnviar, maxEnviar);
        sinRecorte(l);
    }

    /** Hace cuánto: «Ahora», «Hace 5 min», «Hace 3 h», «Ayer», «Hace 2 días». */
    private static String haceCuanto(long ms) {
        long s = Math.max(0, (System.currentTimeMillis() - ms) / 1000);
        if (s < 60) return T.t("Ahora", "Now");
        if (s < 3600) return T.t("Hace " + s / 60 + " min", s / 60 + " min ago");
        if (s < 86400) return T.t("Hace " + s / 3600 + " h", s / 3600 + " h ago");
        long d = s / 86400;
        return d == 1 ? T.t("Ayer", "Yesterday") : T.t("Hace " + d + " días", d + " days ago");
    }

    private static Locale idioma() {
        return T.t("es", "en").equals("es") ? new Locale("es", "MX") : Locale.US;
    }

    /** En la lista de conversaciones: la hora si fue hoy, «Ayer» o el día. */
    private static String horaCorta(long ms) {
        if (ms <= 0) return "";
        Calendar c = Calendar.getInstance(), hoy = Calendar.getInstance();
        c.setTimeInMillis(ms);
        if (mismoDia(c, hoy)) return new SimpleDateFormat("HH:mm").format(new Date(ms));
        hoy.add(Calendar.DAY_OF_YEAR, -1);
        if (mismoDia(c, hoy)) return T.t("Ayer", "Yesterday");
        return new SimpleDateFormat("d MMM", idioma()).format(new Date(ms));
    }

    /** El separador de días entre los mensajes: «Hoy», «Ayer» o la fecha. */
    private static String diaDe(long ms) {
        Calendar c = Calendar.getInstance(), hoy = Calendar.getInstance();
        c.setTimeInMillis(ms);
        if (mismoDia(c, hoy)) return T.t("Hoy", "Today");
        hoy.add(Calendar.DAY_OF_YEAR, -1);
        if (mismoDia(c, hoy)) return T.t("Ayer", "Yesterday");
        return new SimpleDateFormat(T.t("EEEE d 'de' MMMM", "EEEE, MMMM d"), idioma()).format(new Date(ms));
    }

    private static boolean mismoDia(Calendar a, Calendar b) {
        return a.get(Calendar.YEAR) == b.get(Calendar.YEAR) && a.get(Calendar.DAY_OF_YEAR) == b.get(Calendar.DAY_OF_YEAR);
    }

    // ================= Acciones

    private void enviarSolicitud(final Campo campo) {
        final String nombre = campo.valor.trim();
        if (nombre.isEmpty()) {
            campo.foco = true;
            return;
        }
        JsonObject datos = new JsonObject();
        datos.addProperty("nombre", nombre);
        juego.puente("solicitud", datos, (r, error) -> {
            if (error != null) juego.aviso(T.t("No se pudo agregar", "Could not add"), error);
            else {
                campo.poner("");
                juego.aviso(T.t("Solicitud de amistad", "Friend request"), texto(r.getAsJsonObject(), "texto"));
            }
        });
    }

    private void accionAmigo(String op, String uid, final String listo) {
        JsonObject datos = new JsonObject();
        datos.addProperty("uid", uid);
        juego.puente(op, datos, (r, error) -> {
            if (error != null) juego.aviso(T.t("No se pudo", "Could not do it"), error);
            else if (listo != null) juego.aviso("Parabellum", listo);
        });
    }

    private void mensajeA(String uid) {
        // Si ya hay conversación con él, se abre al momento.
        for (JsonObject c : chats()) {
            if (!esGrupo(c) && uid.equals(texto(c, "amigo"))) {
                irAChat(texto(c, "id"));
                return;
            }
        }
        JsonObject datos = new JsonObject();
        datos.addProperty("uid", uid);
        juego.puente("chat_amigo", datos, (r, error) -> {
            if (error != null) juego.aviso(T.t("No se pudo abrir el chat", "Could not open the chat"), error);
            else irAChat(texto(r.getAsJsonObject(), "chat"));
        });
    }

    private void invitar(String uid) {
        JsonObject datos = new JsonObject();
        datos.addProperty("uid", uid);
        juego.puente("invitar", datos, (r, error) -> juego.aviso(error != null ? T.t("No se pudo invitar", "Could not invite") : T.t("Invitación enviada", "Invite sent"), error != null ? error : texto(r.getAsJsonObject(), "texto")));
    }

    private void unirse(String uid, String clave) {
        JsonObject datos = new JsonObject();
        datos.addProperty("uid", uid);
        if (clave != null) datos.addProperty("clave", clave);
        juego.aviso(T.t("Buscando la mejor conexión…", "Looking for the best connection…"), null);
        juego.puente("unirse", datos, (r, error) -> {
            if (error != null) {
                juego.aviso(T.t("No se pudo entrar", "Could not join"), error);
                return;
            }
            JsonObject o = r.getAsJsonObject();
            String ruta = rutaLegible(texto(o, "ruta"), texto(o, "via"));
            String ms = o.has("ms") && !o.get("ms").isJsonNull() ? " (" + o.get("ms").getAsInt() + " ms)" : "";
            juego.aviso(T.t("Entrando con ", "Joining ") + texto(o, "nombre"), ruta + ms);
            juego.plataforma.cerrarPanel();
            juego.plataforma.conectar(texto(o, "direccion"));
        });
    }

    private static String rutaLegible(String ruta, String via) {
        if ("p2p".equals(ruta)) return "relay".equals(via) ? T.t("P2P por relevo", "P2P via relay") : T.t("P2P directo", "Direct P2P");
        if ("lan".equals(ruta)) return T.t("Red local", "Local network");
        if ("playit".equals(ruta)) return "playit.gg";
        return T.t("Servidor", "Server");
    }

    private void favoritoChat(JsonObject chat, boolean activo) {
        JsonObject datos = new JsonObject();
        datos.addProperty("chat", texto(chat, "id"));
        datos.addProperty("activo", activo);
        // Se ve al momento; Parabellum manda el estado nuevo enseguida.
        chat.addProperty("favorito", activo);
        juego.puente("favorito_chat", datos, (r, error) -> {
            if (error != null) juego.aviso(T.t("No se pudo", "Could not do it"), error);
        });
    }

    private void salir(JsonObject chat) {
        final String id = texto(chat, "id");
        JsonObject datos = new JsonObject();
        datos.addProperty("chat", id);
        juego.puente("salir_chat", datos, (r, error) -> {
            if (error != null) juego.aviso(T.t("No se pudo", "Could not do it"), error);
            else if (id.equals(chatAbierto)) {
                chatAbierto = null;
                mensajes = null;
                juego.puente("cerrar_chat", null, null);
            }
        });
    }

    private boolean soyDueno(JsonObject chat) {
        String dueno = texto(chat, "dueno");
        if (dueno == null) return false;
        for (JsonElement m : lista(chat, "miembros")) if (dueno.equals(texto(m.getAsJsonObject(), "uid")) && booleano(m.getAsJsonObject(), "tu")) return true;
        return false;
    }

    private void abrirChat(String id) {
        if (id == null) return;
        if (!id.equals(chatAbierto)) {
            chatAbierto = id;
            mensajes = null;
            editando = null;
            respondiendo = null;
            escribir.poner("");
            scrollMensajes = 0;
        }
        pedirChat(id);
        escribir.foco = true;
    }

    private void pedirChat(String id) {
        JsonObject datos = new JsonObject();
        datos.addProperty("chat", id);
        juego.puente("abrir_chat", datos, null);
    }

    private void enviarMensaje() {
        final String texto = escribir.valor.trim();
        if (chatAbierto == null || texto.isEmpty()) return;
        JsonObject datos = new JsonObject();
        datos.addProperty("chat", chatAbierto);
        datos.addProperty("texto", texto);
        String op = "enviar";
        final JsonObject respuesta = respondiendo;
        if (editando != null) {
            datos.addProperty("mensaje", editando);
            op = "editar";
            editando = null;
        } else if (respuesta != null) {
            datos.addProperty("respuesta", texto(respuesta, "id"));
            respondiendo = null;
        }
        escribir.poner("");
        scrollMensajes = 0;
        juego.puente(op, datos, (r, error) -> {
            if (error != null) {
                escribir.poner(texto);
                respondiendo = respuesta;
                juego.aviso(T.t("No se pudo enviar", "Could not send"), error);
            }
        });
    }

    private void responder(JsonObject m) {
        editando = null;
        JsonObject r = new JsonObject();
        r.addProperty("id", texto(m, "id"));
        r.addProperty("name", texto(m, "name"));
        r.addProperty("text", previa(m));
        r.addProperty("mine", booleano(m, "mine"));
        respondiendo = r;
        escribir.foco = true;
    }

    private void editar(JsonObject m) {
        respondiendo = null;
        editando = texto(m, "id");
        escribir.poner(texto(m, "text"));
        escribir.foco = true;
    }

    private void editarUltimo() {
        if (mensajes == null) return;
        for (int i = mensajes.size() - 1; i >= 0; i--) {
            JsonObject m = mensajes.get(i).getAsJsonObject();
            if (!booleano(m, "mine")) continue;
            if (editable(m)) editar(m);
            return;
        }
    }

    private void confirmarBorrar(final String id) {
        confirmar(T.t("Borrar mensaje", "Delete message"), T.t("Se borra para todos en esta conversación.", "It is deleted for everyone in this conversation."), T.t("Borrar", "Delete"), true, () -> borrar(id));
    }

    private void borrar(String id) {
        JsonObject datos = new JsonObject();
        datos.addProperty("chat", chatAbierto);
        datos.addProperty("mensaje", id);
        juego.puente("borrar", datos, (r, error) -> {
            if (error != null) juego.aviso(T.t("No se pudo borrar", "Could not delete"), error);
        });
    }

    /** Pone tu reacción (o la quita si ya era esa). Se ve al momento; Parabellum la confirma después. */
    private void reaccionar(JsonObject m, String emoji) {
        if (chatAbierto == null) return;
        String actual = null;
        JsonArray lista = lista(m, "reactions");
        for (JsonElement e : lista) if (booleano(e.getAsJsonObject(), "mine")) actual = texto(e.getAsJsonObject(), "emoji");
        String siguiente = emoji.equals(actual) ? null : emoji;
        String yo = texto(objeto(juego.estado, "yo"), "nombre");
        JsonArray nueva = new JsonArray();
        boolean puesta = false;
        for (JsonElement e : lista) {
            // Una copia (deepCopy no existe en la Gson de Minecraft 1.16.5).
            JsonObject original = e.getAsJsonObject(), r = new JsonObject();
            for (Map.Entry<String, JsonElement> campo : original.entrySet()) r.add(campo.getKey(), campo.getValue());
            JsonArray nombres = new JsonArray();
            for (JsonElement n : lista(original, "names")) nombres.add(n);
            r.add("names", nombres);
            int cuenta = (int) numero(r, "count", 1);
            if (booleano(r, "mine")) {
                cuenta--;
                r.addProperty("mine", false);
            }
            if (siguiente != null && siguiente.equals(texto(r, "emoji"))) {
                cuenta++;
                r.addProperty("mine", true);
                if (yo != null) r.getAsJsonArray("names").add(new com.google.gson.JsonPrimitive(yo));
                puesta = true;
            }
            r.addProperty("count", cuenta);
            if (cuenta > 0) nueva.add(r);
        }
        if (siguiente != null && !puesta) {
            JsonObject r = new JsonObject();
            r.addProperty("emoji", siguiente);
            r.addProperty("count", 1);
            r.addProperty("mine", true);
            JsonArray nombres = new JsonArray();
            if (yo != null) nombres.add(new com.google.gson.JsonPrimitive(yo));
            r.add("names", nombres);
            nueva.add(r);
        }
        m.add("reactions", nueva);
        JsonObject datos = new JsonObject();
        datos.addProperty("chat", chatAbierto);
        datos.addProperty("mensaje", texto(m, "id"));
        if (siguiente != null) datos.addProperty("emoji", siguiente);
        juego.puente("reaccionar", datos, (r, error) -> {
            if (error != null) juego.aviso(T.t("No se pudo reaccionar", "Could not react"), error);
        });
    }

    /** Lleva al mensaje citado (si está entre los cargados). */
    private void irAMensaje(String id) {
        if (id == null || mensajes == null) return;
        for (Object[] f : filasPintadas) {
            if (!id.equals(f[0])) continue;
            // Que quede a media altura de la conversación (los mensajes van de abajo hacia arriba).
            scrollMensajes = limitar(scrollMensajes + (H / 2 - 20) - (Double) f[1], maxMensajes);
            return;
        }
        juego.aviso(T.t("Ese mensaje es más viejo que los que se ven aquí.", "That message is older than the ones shown here."), null);
    }

    /** Con un amigo sin conversación: Parabellum la abre (o la crea) y ahí va la captura. */
    private void enviarCapturaAmigo(String uid, final String nombre) {
        JsonObject datos = new JsonObject();
        datos.addProperty("uid", uid);
        juego.puente("chat_amigo", datos, (r, error) -> {
            if (error != null) juego.aviso(T.t("No se pudo enviar", "Could not send"), error);
            else enviarCaptura(texto(r.getAsJsonObject(), "chat"), nombre);
        });
    }

    private void enviarCaptura(final String chat, final String nombre) {
        if (enviando || captura == null) return;
        enviando = true;
        final File archivo = captura;
        Capturas.HILO.execute(() -> {
            try {
                Object[] listo = Capturas.paraChat(archivo);
                JsonObject datos = new JsonObject();
                datos.addProperty("chat", chat);
                datos.addProperty("datos", (String) listo[0]);
                datos.addProperty("ancho", (Integer) listo[1]);
                datos.addProperty("alto", (Integer) listo[2]);
                juego.puente("enviar_imagen", datos, (r, error) -> {
                    enviando = false;
                    if (error != null) juego.aviso(T.t("No se pudo enviar", "Could not send"), error);
                    else {
                        captura = null;
                        juego.aviso(T.t("Captura enviada", "Screenshot sent"), nombre);
                    }
                    marcar();
                });
            } catch (final Exception e) {
                juego.plataforma.enJuego(() -> {
                    enviando = false;
                    juego.aviso(T.t("No se pudo enviar", "Could not send"), e.getMessage());
                });
            }
        });
    }

    /** La foto de un mensaje (se pide a Parabellum la primera vez). */
    private BufferedImage foto(final String id) {
        BufferedImage hecha = fotos.get(id);
        if (hecha != null || pedidas.containsKey(id) || chatAbierto == null) return hecha;
        pedidas.put(id, true);
        JsonObject datos = new JsonObject();
        datos.addProperty("chat", chatAbierto);
        datos.addProperty("mensaje", id);
        juego.puente("imagen", datos, (r, error) -> {
            if (error != null) return;
            decodificar(texto(r.getAsJsonObject(), "datos"), 480, fotos, id);
        });
        return null;
    }

    /** La foto de un grupo (se pide cuando cambia: el estado solo trae su huella). */
    private BufferedImage fotoGrupo(JsonObject chat) {
        String huella = texto(chat, "foto");
        if (huella == null) return null;
        final String clave = texto(chat, "id") + ":" + huella;
        BufferedImage hecha = fotosGrupo.get(clave);
        if (hecha != null || pedidasGrupo.containsKey(clave)) return hecha;
        pedidasGrupo.put(clave, true);
        JsonObject datos = new JsonObject();
        datos.addProperty("chat", texto(chat, "id"));
        juego.puente("foto_grupo", datos, (r, error) -> {
            if (error == null) decodificar(texto(r.getAsJsonObject(), "datos"), 128, fotosGrupo, clave);
        });
        return null;
    }

    private void decodificar(final String url, final int maximo, final Map<String, BufferedImage> destino, final String clave) {
        if (url == null) return;
        Capturas.HILO.execute(() -> {
            try {
                String base64 = url.substring(url.indexOf(',') + 1);
                BufferedImage imagen = ImageIO.read(new ByteArrayInputStream(java.util.Base64.getDecoder().decode(base64)));
                if (imagen != null) {
                    destino.put(clave, Capturas.escalar(imagen, maximo));
                    marcarLuego();
                }
            } catch (Exception ignored) {
            }
        });
    }

    private static double altoFoto(JsonObject m, double maximo) {
        double w = numero(m, "width", 4), h = numero(m, "height", 3);
        double ancho = Math.min(240, maximo - 16);
        return Math.min(220, ancho * h / Math.max(1, w));
    }

    // ================= JSON

    static String texto(JsonObject o, String clave) {
        if (o == null || !o.has(clave) || o.get(clave).isJsonNull()) return null;
        return o.get(clave).getAsString();
    }

    static boolean booleano(JsonObject o, String clave) {
        return o != null && o.has(clave) && o.get(clave).isJsonPrimitive() && o.get(clave).getAsBoolean();
    }

    static double numero(JsonObject o, String clave, double otro) {
        return o != null && o.has(clave) && o.get(clave).isJsonPrimitive() ? o.get(clave).getAsDouble() : otro;
    }

    static JsonObject objeto(JsonObject o, String clave) {
        return o != null && o.has(clave) && o.get(clave).isJsonObject() ? o.getAsJsonObject(clave) : null;
    }

    static JsonArray lista(JsonObject o, String clave) {
        return o != null && o.has(clave) && o.get(clave).isJsonArray() ? o.getAsJsonArray(clave) : new JsonArray();
    }

    private static boolean esGrupo(JsonObject chat) {
        return "grupo".equals(texto(chat, "tipo"));
    }

    /** Lo que se ve de un mensaje al citarlo. */
    private static String previa(JsonObject m) {
        if (booleano(m, "image")) return T.t("Imagen", "Image");
        String texto = texto(m, "text");
        if (texto == null) return "";
        if (texto.startsWith(INVITACION)) return T.t("Invitación", "Invite");
        if (texto.startsWith(COMPARTIR) || texto.startsWith(COMPARTIR2)) return T.t("Instancia compartida", "Shared instance");
        return texto.replaceAll("\\s+", " ").trim();
    }

    static String vistaPrevia(String texto) {
        if (texto == null) return "";
        if (texto.startsWith("📷")) return T.t("Imagen", "Image");
        if (texto.startsWith("📦")) return texto.substring(2).trim();
        if (texto.startsWith("🎮")) return texto.substring(2).trim();
        return texto;
    }
}
