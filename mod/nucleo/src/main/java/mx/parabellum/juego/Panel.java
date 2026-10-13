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
import java.util.Date;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * El panel de Parabellum dentro del juego: amigos (unirse, invitar, mensaje, solicitudes), mensajes
 * (chats y grupos, enviar, editar y borrar) y capturas (mandarlas a un chat). Se dibuja entero con Java2D
 * en unidades de diseño (ver {@link Lienzo}); cada clic se resuelve con las zonas de la última pasada.
 */
final class Panel {
    private static final double W = 880, H = 560, LATERAL = 210, LISTA_CHATS = 240;
    private static final int AMIGOS = 0, MENSAJES = 1, CAPTURAS = 2;
    private static final int PRIMARIO = 0, NORMAL = 1, PELIGRO = 2, SUAVE = 3;
    static final String COMPARTIR = "parabellum:instancia:";
    static final String INVITACION = "parabellum:invitacion:";

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

    private int pestana = AMIGOS;
    private final Campo agregar = new Campo(T.t("Agregar por nombre", "Add by name"), 16);
    private final Campo escribir = new Campo(T.t("Escribe un mensaje…", "Write a message…"), 2000);
    private double scrollAmigos, scrollChats, scrollMensajes, scrollCapturas, scrollEnviar;
    private double maxAmigos, maxChats, maxMensajes, maxCapturas, maxEnviar;
    private String quitar;
    private String chatAbierto;
    private JsonArray mensajes;
    private String editando;
    private final Map<String, BufferedImage> fotos = new ConcurrentHashMap<String, BufferedImage>();
    private final Map<String, Boolean> pedidas = new ConcurrentHashMap<String, Boolean>();
    private File captura;
    private boolean enviando;
    private List<Object[]> menu;
    private double menuX, menuY;
    private boolean registrandoMenu;
    private long parpadeo;

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
        boolean conFoco = agregar.foco || escribir.foco;
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
        ratonX = (px - x) / u;
        ratonY = (py - y) / u;
        Zona nueva = zonaEn(ratonX, ratonY);
        if (nueva != encima) {
            encima = nueva;
            sucio = true;
        }
    }

    boolean clic(double px, double py, int boton) {
        raton(px, py);
        Zona z = zonaEn(ratonX, ratonY);
        boolean habiaMenu = menu != null;
        menu = null;
        sucio = true;
        if (boton == 1) {
            if (z != null && z.derecho != null && !z.deMenu) z.derecho.hacer();
            return true;
        }
        // Con el menú abierto, un clic elige una opción o solo lo cierra.
        if (habiaMenu) {
            if (z != null && z.deMenu && z.accion != null) z.accion.hacer();
            return true;
        }
        agregar.foco = false;
        escribir.foco = false;
        if (z != null && z.accion != null) z.accion.hacer();
        sucio = true;
        return ratonX >= 0 && ratonY >= 0 && ratonX < W && ratonY < H;
    }

    boolean rueda(double px, double py, double cantidad) {
        raton(px, py);
        double paso = cantidad * 48;
        if (pestana == AMIGOS) scrollAmigos = limitar(scrollAmigos - paso, maxAmigos);
        else if (pestana == MENSAJES && ratonX < LATERAL + LISTA_CHATS) scrollChats = limitar(scrollChats - paso, maxChats);
        else if (pestana == MENSAJES) scrollMensajes = limitar(scrollMensajes + paso, maxMensajes);
        else if (captura != null) scrollEnviar = limitar(scrollEnviar - paso, maxEnviar);
        else scrollCapturas = limitar(scrollCapturas - paso, maxCapturas);
        sucio = true;
        return true;
    }

    private static double limitar(double valor, double maximo) {
        return Math.max(0, Math.min(Math.max(0, maximo), valor));
    }

    /** Devuelve true si la tecla se usó (si no, la versión del juego la atiende: Esc cierra). */
    boolean tecla(int tecla, int mods) {
        sucio = true;
        if (tecla == Campo.ESCAPE) {
            if (menu != null) {
                menu = null;
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
        Campo campo = agregar.foco ? agregar : escribir.foco ? escribir : null;
        if (campo == null) return false;
        if (tecla == Campo.ENTER || tecla == Campo.ENTER_NUMERICO) {
            if (campo == agregar) enviarSolicitud();
            else enviarMensaje();
            return true;
        }
        if (tecla == 265 && campo == escribir && escribir.valor.isEmpty()) {
            editarUltimo();
            return true;
        }
        return campo.tecla(tecla, mods, juego.plataforma);
    }

    boolean caracter(char c) {
        Campo campo = agregar.foco ? agregar : escribir.foco ? escribir : null;
        if (campo == null && pestana == MENSAJES && chatAbierto != null) {
            escribir.foco = true;
            campo = escribir;
        }
        if (campo == null) return false;
        sucio = true;
        return campo.caracter(c);
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
        if (!pruebaHecha && "capturas".equals(System.getProperty("parabellum.prueba.pestana"))) {
            pruebaHecha = true;
            pestana = CAPTURAS;
        }
        if (!pruebaHecha && "1".equals(System.getProperty("parabellum.prueba.pestana"))) {
            JsonArray chats = lista(juego.estado, "chats");
            if (chats.size() > 0) {
                pruebaHecha = true;
                irAChat(texto(chats.get(0).getAsJsonObject(), "id"));
            }
        }
        if (pestana == MENSAJES && chatAbierto != null) {
            pedirChat(chatAbierto);
            escribir.foco = true;
        }
    }

    void cerrado() {
        agregar.foco = false;
        escribir.foco = false;
        menu = null;
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
        return menu == null && z.dentro(ratonX, ratonY);
    }

    private void recortar(Lienzo l, double rx, double ry, double rw, double rh) {
        recorte = new double[] {rx, ry, rw, rh};
        l.g.setClip(l.forma(rx, ry, rw, rh, 0));
    }

    private void sinRecorte(Lienzo l) {
        recorte = new double[] {0, 0, W, H};
        l.g.setClip(null);
    }

    private double boton(Lienzo l, String texto, double bx, double by, double h, int estilo, Accion accion) {
        double w = l.ancho(texto, 12.5, true) + 24;
        boolean sobre = zona(bx, by, w, h, accion, null);
        Color fondo = estilo == PRIMARIO ? (sobre ? Lienzo.ACENTO_ENCIMA : Lienzo.ACENTO) : estilo == SUAVE ? (sobre ? Lienzo.TARJETA_ENCIMA : new Color(0, 0, 0, 0)) : (sobre ? Lienzo.TARJETA_ENCIMA : Lienzo.TARJETA);
        l.rect(bx, by, w, h, 8, fondo);
        Color letra = estilo == PRIMARIO ? Color.WHITE : estilo == PELIGRO ? Lienzo.PELIGRO : Lienzo.TEXTO;
        l.texto(texto, bx + 12, by + (h - l.altoLinea(12.5)) / 2, 12.5, true, letra);
        return w;
    }

    /** Botón alineado a la derecha: devuelve su x. */
    private double botonDerecha(Lienzo l, String texto, double derecha, double by, double h, int estilo, Accion accion) {
        double w = l.ancho(texto, 12.5, true) + 24;
        boton(l, texto, derecha - w, by, h, estilo, accion);
        return derecha - w;
    }

    // ================= Dibujo

    private void pintar(Lienzo l, boolean cursorVisible) {
        l.rect(0, 0, W, H, 14, Lienzo.FONDO);
        // Barra lateral
        Shape antes = l.g.getClip();
        l.g.clip(l.forma(0, 0, W, H, 14));
        l.rect(0, 0, LATERAL, H, 0, Lienzo.LATERAL);
        l.g.setClip(antes);
        l.borde(0, 0, W, H, 14, Lienzo.BORDE);
        JsonObject estado = juego.estado;
        String yo = texto(objeto(estado, "yo"), "nombre");
        l.cabeza(yo == null ? "Jugador" : yo, 18, 20, 34);
        l.texto("Parabellum", 62, 20, 16, true, Lienzo.ACENTO_TEXTO);
        l.texto(l.recortar(yo == null ? T.t("Sin sesión", "Not signed in") : yo, LATERAL - 74, 12, false), 62, 40, 12, false, Lienzo.APAGADO);

        int enLinea = 0;
        for (JsonElement a : lista(estado, "amigos")) if (booleano(a.getAsJsonObject(), "enLinea")) enLinea++;
        int sinLeer = 0;
        for (JsonElement c : lista(estado, "chats")) if (booleano(c.getAsJsonObject(), "sinLeer")) sinLeer++;
        int solicitudes = lista(estado, "solicitudes").size();
        pestanaBoton(l, AMIGOS, T.t("Amigos", "Friends"), 82, enLinea > 0 ? String.valueOf(enLinea) : null, solicitudes > 0);
        pestanaBoton(l, MENSAJES, T.t("Mensajes", "Messages"), 126, sinLeer > 0 ? String.valueOf(sinLeer) : null, sinLeer > 0);
        pestanaBoton(l, CAPTURAS, T.t("Capturas", "Screenshots"), 170, null, false);

        // Estado de la conexión y cómo cerrar
        boolean conectado = juego.conectado();
        l.circulo(20, H - 46, 8, conectado ? Lienzo.EXITO : Lienzo.PELIGRO);
        l.texto(conectado ? T.t("Conectado a Parabellum", "Connected to Parabellum") : T.t("Sin Parabellum", "No Parabellum"), 34, H - 50, 12, false, Lienzo.APAGADO);
        l.texto(juego.plataforma.descripcion() + " · " + T.t("Esc para cerrar", "Esc to close"), 20, H - 30, 11, false, Lienzo.TENUE);

        if (!conectado) {
            vacio(l, juego.puenteDisponible() ? T.t("Esperando a Parabellum…", "Waiting for Parabellum…") : T.t("Abre este juego desde Parabellum para ver a tus amigos.", "Open this game from Parabellum to see your friends."), null);
        } else if (!booleano(estado, "sesion")) {
            vacio(l, T.t("Inicia sesión con Google en Parabellum para ver a tus amigos.", "Sign in with Google in Parabellum to see your friends."), new Object[] {T.t("Abrir Parabellum", "Open Parabellum"), (Accion) () -> juego.puente("abrir_parabellum", null, null)});
        } else if (pestana == AMIGOS) {
            pintarAmigos(l, cursorVisible);
        } else if (pestana == MENSAJES) {
            pintarMensajes(l, cursorVisible);
        } else {
            pintarCapturas(l);
        }
        sinRecorte(l);
        if (menu != null) pintarMenu(l);
    }

    private void pestanaBoton(Lienzo l, final int cual, String nombre, double by, String cuenta, boolean resaltar) {
        boolean activa = pestana == cual;
        boolean sobre = zona(10, by, LATERAL - 20, 38, () -> cambiarPestana(cual), null);
        if (activa || sobre) l.rect(10, by, LATERAL - 20, 38, 9, activa ? Lienzo.TARJETA : new Color(0x2a, 0x28, 0x32, 120));
        if (activa) l.rect(10, by + 10, 3, 18, 1.5, Lienzo.ACENTO);
        l.texto(nombre, 24, by + 10, 14, true, activa ? Lienzo.TEXTO : Lienzo.APAGADO);
        if (cuenta != null) {
            double w = Math.max(22, l.ancho(cuenta, 11, true) + 12);
            l.rect(LATERAL - 20 - w - 8, by + 9, w, 20, 10, resaltar ? Lienzo.ACENTO : Lienzo.TARJETA_ENCIMA);
            l.texto(cuenta, LATERAL - 20 - w - 8 + (w - l.ancho(cuenta, 11, true)) / 2, by + 11, 11, true, Color.WHITE);
        } else if (resaltar) {
            l.circulo(LATERAL - 36, by + 15, 8, Lienzo.ACENTO);
        }
    }

    private void cambiarPestana(int cual) {
        pestana = cual;
        menu = null;
        captura = cual == CAPTURAS ? captura : null;
        if (cual == MENSAJES && chatAbierto != null) {
            pedirChat(chatAbierto);
            escribir.foco = true;
        }
    }

    private void vacio(Lienzo l, String texto, Object[] boton) {
        double cx = LATERAL + (W - LATERAL) / 2;
        List<String> lineas = l.lineas(texto, 380, 14, false);
        double ty = H / 2 - 30;
        for (String linea : lineas) {
            l.texto(linea, cx - l.ancho(linea, 14, false) / 2, ty, 14, false, Lienzo.APAGADO);
            ty += 20;
        }
        if (boton != null) {
            double w = l.ancho((String) boton[0], 12.5, true) + 24;
            boton(l, (String) boton[0], cx - w / 2, ty + 12, 34, PRIMARIO, (Accion) boton[1]);
        }
    }

    private void titulo(Lienzo l, String texto) {
        l.texto(texto, LATERAL + 24, 22, 19, true, Lienzo.TEXTO);
    }

    // ---------- Amigos

    private void pintarAmigos(Lienzo l, boolean cursorVisible) {
        titulo(l, T.t("Amigos", "Friends"));
        double derecha = W - 24;
        double bx = botonDerecha(l, T.t("Agregar", "Add"), derecha, 18, 34, NORMAL, this::enviarSolicitud);
        zona(bx - 208, 18, 200, 34, () -> agregar.foco = true, null);
        agregar.dibujar(l, bx - 208, 18, 200, 34, cursorVisible);

        double arriba = 68, abajo = H - 14;
        recortar(l, LATERAL, arriba, W - LATERAL, abajo - arriba);
        double ty = arriba - scrollAmigos;
        JsonArray solicitudes = lista(juego.estado, "solicitudes");
        if (solicitudes.size() > 0) {
            l.texto(T.t("SOLICITUDES", "REQUESTS"), LATERAL + 24, ty + 4, 11, true, Lienzo.TENUE);
            ty += 26;
            for (JsonElement e : solicitudes) {
                final JsonObject s = e.getAsJsonObject();
                final String uid = texto(s, "uid");
                l.rect(LATERAL + 16, ty, W - LATERAL - 32, 52, 10, Lienzo.TARJETA);
                l.cabeza(texto(s, "nombre"), LATERAL + 28, ty + 10, 32);
                l.texto(texto(s, "nombre"), LATERAL + 72, ty + 8, 14, true, Lienzo.TEXTO);
                l.texto(T.t("Quiere ser tu amigo", "Wants to be your friend"), LATERAL + 72, ty + 27, 12, false, Lienzo.APAGADO);
                double bx2 = botonDerecha(l, T.t("Rechazar", "Decline"), W - 28, ty + 10, 32, NORMAL, () -> accionAmigo("rechazar", uid, null));
                botonDerecha(l, T.t("Aceptar", "Accept"), bx2 - 8, ty + 10, 32, PRIMARIO, () -> accionAmigo("aceptar", uid, T.t("Ahora son amigos.", "You are now friends.")));
                ty += 60;
            }
            ty += 10;
        }
        JsonArray amigos = lista(juego.estado, "amigos");
        int enLinea = 0;
        for (JsonElement a : amigos) if (booleano(a.getAsJsonObject(), "enLinea")) enLinea++;
        l.texto(T.t("AMIGOS · " + enLinea + " EN LÍNEA", "FRIENDS · " + enLinea + " ONLINE"), LATERAL + 24, ty + 4, 11, true, Lienzo.TENUE);
        ty += 26;
        if (amigos.size() == 0) {
            l.texto(T.t("Todavía no tienes amigos: agrégalos por su nombre de Parabellum.", "You have no friends yet: add them by their Parabellum name."), LATERAL + 24, ty + 6, 13, false, Lienzo.APAGADO);
            ty += 30;
        }
        boolean puedoInvitar = booleano(juego.estado, "puedoInvitar");
        // En línea primero (Parabellum ya los manda así: con mundo abierto, jugando, en línea).
        for (JsonElement e : amigos) {
            final JsonObject a = e.getAsJsonObject();
            final String uid = texto(a, "uid");
            final String nombre = texto(a, "nombre");
            boolean online = booleano(a, "enLinea");
            boolean sobreFila = ratonY >= ty && ratonY < ty + 56 && ratonX > LATERAL;
            l.rect(LATERAL + 16, ty, W - LATERAL - 32, 56, 10, sobreFila ? Lienzo.TARJETA : new Color(0x2a, 0x28, 0x32, 110));
            l.cabeza(nombre, LATERAL + 28, ty + 11, 34);
            if (!online) l.rect(LATERAL + 28, ty + 11, 34, 34, 6, new Color(0x1e, 0x1d, 0x23, 150));
            l.circulo(LATERAL + 54, ty + 37, 10, online ? Lienzo.EXITO : Lienzo.TENUE);
            l.texto(l.recortar(nombre, 200, 14, true), LATERAL + 74, ty + 10, 14, true, online ? Lienzo.TEXTO : Lienzo.APAGADO);
            l.texto(l.recortar(estadoAmigo(a), 260, 12, false), LATERAL + 74, ty + 30, 12, false, Lienzo.APAGADO);
            double bx3 = W - 28;
            if (uid.equals(quitar)) {
                bx3 = botonDerecha(l, T.t("Cancelar", "Cancel"), bx3, ty + 12, 32, NORMAL, () -> quitar = null);
                bx3 = botonDerecha(l, T.t("Quitar", "Remove"), bx3 - 8, ty + 12, 32, PELIGRO, () -> {
                    quitar = null;
                    accionAmigo("quitar_amigo", uid, null);
                });
                String pregunta = T.t("¿Quitar a " + nombre + "?", "Remove " + nombre + "?");
                l.texto(pregunta, bx3 - 12 - l.ancho(pregunta, 12.5, true), ty + 19, 12.5, true, Lienzo.PELIGRO);
            } else {
                bx3 = botonDerecha(l, "···", bx3, ty + 12, 32, SUAVE, () -> quitar = uid);
                bx3 = botonDerecha(l, T.t("Mensaje", "Message"), bx3 - 6, ty + 12, 32, NORMAL, () -> mensajeA(uid));
                if (puedoInvitar && online) bx3 = botonDerecha(l, T.t("Invitar", "Invite"), bx3 - 6, ty + 12, 32, NORMAL, () -> invitar(uid));
                if (booleano(a, "unirse")) botonDerecha(l, T.t("Unirse", "Join"), bx3 - 6, ty + 12, 32, PRIMARIO, () -> unirse(uid, null));
            }
            ty += 62;
        }

        // Grupos, debajo de los amigos (como en el panel derecho de Parabellum): uno abre su conversación.
        java.util.List<JsonObject> grupos = new java.util.ArrayList<JsonObject>();
        for (JsonElement e : lista(juego.estado, "chats")) if ("grupo".equals(texto(e.getAsJsonObject(), "tipo"))) grupos.add(e.getAsJsonObject());
        if (!grupos.isEmpty()) {
            ty += 10;
            l.texto(T.t("GRUPOS · " + grupos.size(), "GROUPS · " + grupos.size()), LATERAL + 24, ty + 4, 11, true, Lienzo.TENUE);
            ty += 26;
            for (final JsonObject g : grupos) {
                final String id = texto(g, "id");
                String nombre = texto(g, "nombre");
                boolean sobre = zona(LATERAL + 16, ty, W - LATERAL - 32, 52, () -> irAChat(id), null);
                l.rect(LATERAL + 16, ty, W - LATERAL - 32, 52, 10, sobre ? Lienzo.TARJETA : new Color(0x2a, 0x28, 0x32, 110));
                l.rect(LATERAL + 28, ty + 10, 32, 32, 8, Lienzo.ACENTO_SUAVE);
                String inicial = nombre == null || nombre.isEmpty() ? "#" : nombre.substring(0, 1).toUpperCase();
                l.texto(inicial, LATERAL + 44 - l.ancho(inicial, 14, true) / 2, ty + 17, 14, true, Lienzo.ACENTO_TEXTO);
                boolean nuevo = booleano(g, "sinLeer");
                l.texto(l.recortar(nombre, 300, 14, true), LATERAL + 74, ty + 8, 14, true, Lienzo.TEXTO);
                String ultimo = vistaPrevia(texto(g, "ultimo"));
                if (texto(g, "ultimoNombre") != null && !ultimo.isEmpty()) ultimo = texto(g, "ultimoNombre") + ": " + ultimo;
                if (ultimo.isEmpty()) ultimo = T.t("Grupo", "Group");
                l.texto(l.recortar(ultimo, W - LATERAL - 160, 12, false), LATERAL + 74, ty + 28, 12, false, nuevo ? Lienzo.TEXTO : Lienzo.APAGADO);
                if (nuevo) l.circulo(W - 46, ty + 21, 10, Lienzo.ACENTO);
                ty += 58;
            }
        }
        maxAmigos = Math.max(0, ty + scrollAmigos - abajo + 10);
        scrollAmigos = limitar(scrollAmigos, maxAmigos);
    }

    private static String estadoAmigo(JsonObject a) {
        if (!booleano(a, "enLinea")) return T.t("Desconectado", "Offline");
        String pack = texto(a, "pack");
        if (booleano(a, "mundo")) return T.t("Mundo abierto", "Open world") + (pack != null ? " · " + pack : "");
        String hostea = texto(a, "hostea");
        String servidor = texto(a, "servidor");
        if (booleano(a, "jugando") && servidor != null) return T.t("Jugando en ", "Playing on ") + servidor;
        if (booleano(a, "jugando")) return T.t("Jugando", "Playing") + (pack != null ? " " + pack : "");
        if (hostea != null) return T.t("Servidor encendido: ", "Server on: ") + hostea;
        return T.t("En línea", "Online");
    }

    // ---------- Mensajes

    private void pintarMensajes(Lienzo l, boolean cursorVisible) {
        // Conversaciones
        double lx = LATERAL, lw = LISTA_CHATS;
        l.rect(lx + lw, 0, 1, H, 0, Lienzo.BORDE);
        l.texto(T.t("Mensajes", "Messages"), lx + 20, 22, 19, true, Lienzo.TEXTO);
        double arriba = 64, abajo = H - 10;
        recortar(l, lx, arriba, lw, abajo - arriba);
        double ty = arriba - scrollChats;
        JsonArray chats = lista(juego.estado, "chats");
        if (chats.size() == 0) l.texto(T.t("Sin conversaciones", "No conversations"), lx + 20, ty + 8, 13, false, Lienzo.APAGADO);
        for (JsonElement e : chats) {
            final JsonObject c = e.getAsJsonObject();
            final String id = texto(c, "id");
            boolean abierto = id.equals(chatAbierto);
            boolean sobre = zona(lx + 8, ty, lw - 16, 54, () -> abrirChat(id), null);
            if (abierto || sobre) l.rect(lx + 8, ty, lw - 16, 54, 9, abierto ? Lienzo.TARJETA : new Color(0x2a, 0x28, 0x32, 120));
            boolean grupo = "grupo".equals(texto(c, "tipo"));
            String nombre = texto(c, "nombre");
            if (grupo) {
                l.rect(lx + 16, ty + 11, 32, 32, 8, Lienzo.ACENTO_SUAVE);
                String inicial = nombre == null || nombre.isEmpty() ? "#" : nombre.substring(0, 1).toUpperCase();
                l.texto(inicial, lx + 32 - l.ancho(inicial, 14, true) / 2, ty + 18, 14, true, Lienzo.ACENTO_TEXTO);
            } else {
                l.cabeza(nombre, lx + 16, ty + 11, 32);
            }
            boolean nuevo = booleano(c, "sinLeer");
            l.texto(l.recortar(nombre, lw - 90, 13, true), lx + 56, ty + 9, 13, true, Lienzo.TEXTO);
            String ultimo = vistaPrevia(texto(c, "ultimo"));
            if (grupo && texto(c, "ultimoNombre") != null && !ultimo.isEmpty()) ultimo = texto(c, "ultimoNombre") + ": " + ultimo;
            l.texto(l.recortar(ultimo, lw - 90, 12, false), lx + 56, ty + 28, 12, false, nuevo ? Lienzo.TEXTO : Lienzo.APAGADO);
            if (nuevo) l.circulo(lx + lw - 26, ty + 22, 9, Lienzo.ACENTO);
            ty += 58;
        }
        maxChats = Math.max(0, ty + scrollChats - abajo + 10);
        sinRecorte(l);

        // Conversación abierta
        double cx = lx + lw, cw = W - cx;
        if (chatAbierto == null) {
            String t = T.t("Elige una conversación", "Pick a conversation");
            l.texto(t, cx + (cw - l.ancho(t, 14, false)) / 2, H / 2 - 10, 14, false, Lienzo.APAGADO);
            return;
        }
        JsonObject chat = null;
        for (JsonElement e : chats) if (chatAbierto.equals(texto(e.getAsJsonObject(), "id"))) chat = e.getAsJsonObject();
        boolean grupo = chat != null && "grupo".equals(texto(chat, "tipo"));
        l.texto(l.recortar(chat == null ? "" : texto(chat, "nombre"), cw - 40, 16, true), cx + 20, 24, 16, true, Lienzo.TEXTO);
        l.rect(cx, 60, cw, 1, 0, Lienzo.BORDE);

        double campoY = H - 54;
        double areaArriba = 62, areaAbajo = campoY - 8;
        recortar(l, cx, areaArriba, cw, areaAbajo - areaArriba);
        if (mensajes == null) {
            l.texto(T.t("Cargando…", "Loading…"), cx + 20, areaArriba + 16, 13, false, Lienzo.APAGADO);
        } else if (mensajes.size() == 0) {
            l.texto(T.t("Todavía no hay mensajes. ¡Saluda!", "No messages yet. Say hi!"), cx + 20, areaArriba + 16, 13, false, Lienzo.APAGADO);
        } else {
            pintarBurbujas(l, cx, cw, areaArriba, areaAbajo, grupo);
        }
        sinRecorte(l);

        // Escribir
        double enviarX = botonDerecha(l, editando != null ? T.t("Guardar", "Save") : T.t("Enviar", "Send"), W - 16, campoY, 38, PRIMARIO, this::enviarMensaje);
        zona(cx + 16, campoY, enviarX - cx - 24, 38, () -> escribir.foco = true, null);
        escribir.dibujar(l, cx + 16, campoY, enviarX - cx - 24, 38, cursorVisible);
        if (editando != null) l.texto(T.t("Editando · Esc para cancelar", "Editing · Esc to cancel"), cx + 18, campoY - 16, 11, false, Lienzo.ACENTO_TEXTO);
    }

    /** Las burbujas, de abajo hacia arriba (lo más nuevo abajo). */
    private void pintarBurbujas(Lienzo l, double cx, double cw, double arriba, double abajo, boolean grupo) {
        double maxBurbuja = Math.min(320, cw * 0.72);
        List<Object[]> bloques = new ArrayList<Object[]>();
        double total = 0;
        String anterior = null;
        for (JsonElement e : mensajes) {
            JsonObject m = e.getAsJsonObject();
            boolean mio = booleano(m, "mine");
            boolean continua = texto(m, "from") != null && texto(m, "from").equals(anterior);
            anterior = texto(m, "from");
            double alto = altoBurbuja(l, m, maxBurbuja) + (grupo && !mio && !continua ? 16 : 0) + (continua ? 4 : 10);
            bloques.add(new Object[] {m, alto, continua});
            total += alto;
        }
        double visible = abajo - arriba;
        maxMensajes = Math.max(0, total - visible + 12);
        scrollMensajes = limitar(scrollMensajes, maxMensajes);
        double ty = abajo - total - 6 + scrollMensajes;
        if (total < visible) ty = arriba + 8;
        for (Object[] b : bloques) {
            JsonObject m = (JsonObject) b[0];
            double alto = (Double) b[1];
            boolean continua = (Boolean) b[2];
            if (ty + alto >= arriba && ty <= abajo) pintarBurbuja(l, m, cx, cw, ty + (continua ? 4 : 10), maxBurbuja, grupo && !continua);
            ty += alto;
        }
    }

    private double altoBurbuja(Lienzo l, JsonObject m, double maximo) {
        if (booleano(m, "image")) return altoFoto(m, maximo) + 22;
        String texto = texto(m, "text");
        if (texto != null && (texto.startsWith(INVITACION) || texto.startsWith(COMPARTIR))) return 74;
        return l.lineas(texto, maximo - 24, 13, false).size() * l.altoLinea(13) + 30;
    }

    private static double altoFoto(JsonObject m, double maximo) {
        double w = numero(m, "width", 4), h = numero(m, "height", 3);
        double ancho = Math.min(240, maximo - 16);
        return Math.min(220, ancho * h / Math.max(1, w));
    }

    private void pintarBurbuja(Lienzo l, final JsonObject m, double cx, double cw, double ty, double maximo, boolean autor) {
        final boolean mio = booleano(m, "mine");
        final String id = texto(m, "id");
        if (autor && !mio) {
            l.texto(texto(m, "name"), cx + 22, ty, 11, true, Lienzo.TENUE);
            ty += 16;
        }
        String texto = texto(m, "text");
        String hora = new SimpleDateFormat("HH:mm").format(new Date((long) numero(m, "at", 0)));
        if (booleano(m, "edited")) hora = T.t("editado · ", "edited · ") + hora;
        Color fondo = mio ? Lienzo.ACENTO : Lienzo.TARJETA;
        Color letra = mio ? Color.WHITE : Lienzo.TEXTO;
        Accion derecho = mio ? () -> menuMensaje(m) : null;
        if (booleano(m, "image")) {
            double fotoAlto = altoFoto(m, maximo);
            double fotoAncho = Math.min(240, maximo - 16);
            double bx = mio ? cx + cw - 18 - fotoAncho - 12 : cx + 18;
            l.rect(bx, ty, fotoAncho + 12, fotoAlto + 22, 12, fondo);
            zona(bx, ty, fotoAncho + 12, fotoAlto + 22, null, derecho);
            BufferedImage foto = foto(id);
            if (foto != null) l.imagen(foto, bx + 6, ty + 6, fotoAncho, fotoAlto, 8);
            else l.rect(bx + 6, ty + 6, fotoAncho, fotoAlto, 8, new Color(0, 0, 0, 60));
            l.texto(hora, bx + fotoAncho + 6 - l.ancho(hora, 10, false), ty + fotoAlto + 8, 10, false, mio ? new Color(255, 255, 255, 190) : Lienzo.TENUE);
            return;
        }
        if (texto != null && (texto.startsWith(INVITACION) || texto.startsWith(COMPARTIR))) {
            pintarTarjeta(l, m, texto, cx, cw, ty, mio, derecho);
            return;
        }
        List<String> lineas = l.lineas(texto, maximo - 24, 13, false);
        double ancho = l.ancho(hora, 10, false);
        for (String linea : lineas) ancho = Math.max(ancho, l.ancho(linea, 13, false));
        double w = ancho + 24, h = lineas.size() * l.altoLinea(13) + 26;
        double bx = mio ? cx + cw - 18 - w : cx + 18;
        l.rect(bx, ty, w, h, 12, fondo);
        zona(bx, ty, w, h, null, derecho);
        double ly = ty + 7;
        for (String linea : lineas) {
            l.texto(linea, bx + 12, ly, 13, false, letra);
            ly += l.altoLinea(13);
        }
        l.texto(hora, bx + w - 12 - l.ancho(hora, 10, false), ly + 1, 10, false, mio ? new Color(255, 255, 255, 190) : Lienzo.TENUE);
    }

    /** Invitación (con «Unirse») o instancia compartida (se abre en Parabellum). */
    @SuppressWarnings("deprecation")
    private void pintarTarjeta(Lienzo l, JsonObject m, String texto, double cx, double cw, double ty, boolean mio, Accion derecho) {
        double w = 280, h = 64;
        double bx = mio ? cx + cw - 18 - w : cx + 18;
        l.rect(bx, ty, w, h, 12, mio ? new Color(0x5e, 0x3a, 0xc0) : Lienzo.TARJETA);
        l.borde(bx, ty, w, h, 12, Lienzo.ACENTO);
        zona(bx, ty, w, h, null, derecho);
        if (texto.startsWith(INVITACION)) {
            JsonObject datos;
            try {
                datos = new JsonParser().parse(texto.substring(INVITACION.length())).getAsJsonObject();
            } catch (Exception e) {
                datos = new JsonObject();
            }
            boolean servidor = "servidor".equals(texto(datos, "t"));
            l.texto(servidor ? T.t("Invitación a su servidor", "Server invite") : T.t("Invitación a su mundo", "World invite"), bx + 14, ty + 10, 11, true, Lienzo.ACENTO_TEXTO);
            l.texto(l.recortar(texto(datos, "n"), w - 120, 14, true), bx + 14, ty + 28, 14, true, Color.WHITE);
            if (!mio) {
                final String anfitrion = texto(m, "from");
                final String clave = servidor ? texto(datos, "k") : null;
                botonDerecha(l, T.t("Unirse", "Join"), bx + w - 12, ty + 16, 32, PRIMARIO, () -> unirse(anfitrion, clave));
            }
        } else {
            l.texto(T.t("Instancia compartida", "Shared instance"), bx + 14, ty + 10, 11, true, Lienzo.ACENTO_TEXTO);
            l.texto(T.t("Ábrela en Parabellum", "Open it in Parabellum"), bx + 14, ty + 28, 14, true, Color.WHITE);
            botonDerecha(l, T.t("Abrir", "Open"), bx + w - 12, ty + 16, 32, NORMAL, () -> juego.puente("abrir_parabellum", null, null));
        }
    }

    private void menuMensaje(final JsonObject m) {
        menu = new ArrayList<Object[]>();
        final String id = texto(m, "id");
        String texto = texto(m, "text");
        boolean editable = !booleano(m, "image") && texto != null && !texto.startsWith(INVITACION) && !texto.startsWith(COMPARTIR);
        if (editable) menu.add(new Object[] {T.t("Editar", "Edit"), (Accion) () -> {
            editando = id;
            escribir.poner(texto(m, "text"));
            escribir.foco = true;
        }});
        menu.add(new Object[] {T.t("Borrar", "Delete"), (Accion) () -> borrar(id)});
        menuX = ratonX;
        menuY = ratonY;
    }

    private void pintarMenu(Lienzo l) {
        double w = 150, h = menu.size() * 32 + 8;
        double mx = Math.min(menuX, W - w - 8), my = Math.min(menuY, H - h - 8);
        l.rect(mx, my, w, h, 10, Lienzo.TARJETA);
        l.borde(mx, my, w, h, 10, Lienzo.BORDE);
        List<Object[]> opciones = menu;
        menu = null;
        registrandoMenu = true;
        double oy = my + 4;
        for (Object[] o : opciones) {
            final Accion accion = (Accion) o[1];
            boolean sobre = zona(mx + 4, oy, w - 8, 32, accion, null);
            if (sobre) l.rect(mx + 4, oy, w - 8, 32, 7, Lienzo.TARJETA_ENCIMA);
            boolean borrar = T.t("Borrar", "Delete").equals(o[0]);
            l.texto((String) o[0], mx + 14, oy + 8, 13, false, borrar ? Lienzo.PELIGRO : Lienzo.TEXTO);
            oy += 32;
        }
        registrandoMenu = false;
        menu = opciones;
    }

    // ---------- Capturas

    private void pintarCapturas(Lienzo l) {
        titulo(l, T.t("Capturas", "Screenshots"));
        List<File> lista = juego.capturas.ultimas();
        if (captura != null) {
            pintarEnviarCaptura(l);
            return;
        }
        l.texto(T.t("Pulsa F2 en el juego para sacar una. Elige una para mandarla.", "Press F2 in the game to take one. Pick one to send it."), LATERAL + 24, 48, 12, false, Lienzo.APAGADO);
        if (lista.isEmpty()) {
            vacio(l, T.t("Todavía no hay capturas.", "No screenshots yet."), null);
            return;
        }
        double arriba = 74, abajo = H - 14;
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
            // Cuándo se sacó, en una etiqueta sobre la imagen.
            String cuando = haceCuanto(f.lastModified());
            double ew = l.ancho(cuando, 11, true) + 14;
            l.rect(cx + 8, cy + ch - 28, ew, 20, 6, new Color(0x14, 0x13, 0x18, 200));
            l.texto(cuando, cx + 15, cy + ch - 25, 11, true, Lienzo.TEXTO);
            if (sobre) {
                l.borde(cx, cy, cw, ch, 8, Lienzo.ACENTO);
                String mandar = T.t("Mandar", "Send");
                double mw = l.ancho(mandar, 12, true) + 20;
                l.rect(cx + cw - mw - 8, cy + ch - 32, mw, 24, 7, Lienzo.ACENTO);
                l.texto(mandar, cx + cw - mw + 2, cy + ch - 28, 12, true, Color.WHITE);
            }
        }
        double filas = Math.ceil(lista.size() / columnas);
        maxCapturas = Math.max(0, filas * (ch + gap) - (abajo - arriba) + 10);
    }

    private void pintarEnviarCaptura(Lienzo l) {
        double px = LATERAL + 24, py = 60, pw = 360, ph = pw * 9 / 16;
        BufferedImage mini = juego.capturas.miniatura(captura, this::marcarLuego);
        if (mini != null) l.imagen(mini, px, py, pw, ph, 10);
        l.texto(l.recortar(captura.getName(), pw, 11, false), px, py + ph + 8, 11, false, Lienzo.TENUE);
        boton(l, T.t("Volver", "Back"), px, py + ph + 30, 32, NORMAL, () -> captura = null);
        double lx = px + pw + 24, lw = W - lx - 20;
        l.texto(enviando ? T.t("Enviando…", "Sending…") : T.t("Enviar a", "Send to"), lx, py, 13, true, Lienzo.APAGADO);
        double arriba = py + 24, abajo = H - 16;
        recortar(l, lx, arriba, lw, abajo - arriba);
        double ty = arriba - scrollEnviar;
        // Primero las conversaciones (chats y grupos); después los amigos con los que todavía no hay una.
        java.util.Set<String> conChat = new java.util.HashSet<String>();
        for (JsonElement e : lista(juego.estado, "chats")) {
            final JsonObject c = e.getAsJsonObject();
            final String id = texto(c, "id");
            final String nombre = texto(c, "nombre");
            boolean grupo = "grupo".equals(texto(c, "tipo"));
            if (!grupo && nombre != null) conChat.add(nombre.toLowerCase());
            filaEnviar(l, lx, ty, lw, nombre, grupo, () -> enviarCaptura(id, nombre));
            ty += 46;
        }
        for (JsonElement e : lista(juego.estado, "amigos")) {
            final JsonObject a = e.getAsJsonObject();
            final String uid = texto(a, "uid");
            final String nombre = texto(a, "nombre");
            if (nombre == null || conChat.contains(nombre.toLowerCase())) continue;
            filaEnviar(l, lx, ty, lw, nombre, false, () -> enviarCapturaAmigo(uid, nombre));
            ty += 46;
        }
        if (ty == arriba - scrollEnviar) l.texto(T.t("Agrega amigos para mandarles capturas.", "Add friends to send them screenshots."), lx, ty + 8, 12, false, Lienzo.APAGADO);
        maxEnviar = Math.max(0, ty + scrollEnviar - abajo + 6);
        scrollEnviar = limitar(scrollEnviar, maxEnviar);
        sinRecorte(l);
    }

    /** Una fila de «Enviar a»: el grupo con su inicial o la cabeza del amigo. */
    private void filaEnviar(Lienzo l, double x, double y, double w, String nombre, boolean grupo, Accion accion) {
        boolean sobre = zona(x, y, w, 40, enviando ? null : accion, null);
        l.rect(x, y, w, 40, 8, sobre && !enviando ? Lienzo.TARJETA_ENCIMA : Lienzo.TARJETA);
        if (grupo) {
            l.rect(x + 8, y + 8, 24, 24, 6, Lienzo.ACENTO_SUAVE);
            String inicial = nombre == null || nombre.isEmpty() ? "#" : nombre.substring(0, 1).toUpperCase();
            l.texto(inicial, x + 20 - l.ancho(inicial, 12, true) / 2, y + 12, 12, true, Lienzo.ACENTO_TEXTO);
        } else {
            l.cabeza(nombre, x + 8, y + 8, 24);
        }
        l.texto(l.recortar(nombre, w - 50, 13, true), x + 40, y + 11, 13, true, enviando ? Lienzo.APAGADO : Lienzo.TEXTO);
    }

    /** Hace cuánto (para las capturas): «Ahora», «Hace 5 min», «Hace 3 h», «Ayer», «Hace 2 días». */
    private static String haceCuanto(long ms) {
        long s = Math.max(0, (System.currentTimeMillis() - ms) / 1000);
        if (s < 60) return T.t("Ahora", "Now");
        if (s < 3600) return T.t("Hace " + s / 60 + " min", s / 60 + " min ago");
        if (s < 86400) return T.t("Hace " + s / 3600 + " h", s / 3600 + " h ago");
        long d = s / 86400;
        return d == 1 ? T.t("Ayer", "Yesterday") : T.t("Hace " + d + " días", d + " days ago");
    }

    // ================= Acciones

    private void enviarSolicitud() {
        final String nombre = agregar.valor.trim();
        if (nombre.isEmpty()) {
            agregar.foco = true;
            return;
        }
        JsonObject datos = new JsonObject();
        datos.addProperty("nombre", nombre);
        juego.puente("solicitud", datos, (r, error) -> {
            if (error != null) juego.aviso(T.t("No se pudo agregar", "Could not add"), error);
            else {
                agregar.poner("");
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

    private void abrirChat(String id) {
        if (!id.equals(chatAbierto)) {
            chatAbierto = id;
            mensajes = null;
            editando = null;
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
        if (editando != null) {
            datos.addProperty("mensaje", editando);
            op = "editar";
            editando = null;
        }
        escribir.poner("");
        scrollMensajes = 0;
        juego.puente(op, datos, (r, error) -> {
            if (error != null) {
                escribir.poner(texto);
                juego.aviso(T.t("No se pudo enviar", "Could not send"), error);
            }
        });
    }

    private void editarUltimo() {
        if (mensajes == null) return;
        for (int i = mensajes.size() - 1; i >= 0; i--) {
            JsonObject m = mensajes.get(i).getAsJsonObject();
            if (!booleano(m, "mine")) continue;
            String texto = texto(m, "text");
            if (booleano(m, "image") || texto == null || texto.startsWith(INVITACION) || texto.startsWith(COMPARTIR)) return;
            editando = texto(m, "id");
            escribir.poner(texto);
            return;
        }
    }

    private void borrar(String id) {
        JsonObject datos = new JsonObject();
        datos.addProperty("chat", chatAbierto);
        datos.addProperty("mensaje", id);
        juego.puente("borrar", datos, (r, error) -> {
            if (error != null) juego.aviso(T.t("No se pudo borrar", "Could not delete"), error);
        });
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
            final String url = texto(r.getAsJsonObject(), "datos");
            Capturas.HILO.execute(() -> {
                try {
                    String base64 = url.substring(url.indexOf(',') + 1);
                    BufferedImage imagen = ImageIO.read(new ByteArrayInputStream(java.util.Base64.getDecoder().decode(base64)));
                    if (imagen != null) {
                        fotos.put(id, Capturas.escalar(imagen, 480));
                        marcarLuego();
                    }
                } catch (Exception ignored) {
                }
            });
        });
        return null;
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

    static String vistaPrevia(String texto) {
        if (texto == null) return "";
        if (texto.startsWith("📷")) return T.t("Imagen", "Image");
        if (texto.startsWith("📦")) return texto.substring(2).trim();
        if (texto.startsWith("🎮")) return texto.substring(2).trim();
        return texto;
    }
}
