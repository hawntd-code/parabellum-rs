package mx.parabellum.juego;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;

/**
 * Lo que la versión del juego llama: {@link #iniciar} al cargar, {@link #tick} cada tick del cliente,
 * {@link #tecla} al pulsar la tecla de Parabellum, {@link #panel()} desde su pantalla y {@link #avisos}
 * al dibujar la interfaz del juego.
 */
public final class Juego {
    private static Juego actual;

    final Plataforma plataforma;
    final Capturas capturas;
    final Avisos avisos = new Avisos();
    final Panel panel;
    private final Puente puente;
    /** Lo último que mandó Parabellum (amigos, solicitudes, conversaciones). */
    JsonObject estado = new JsonObject();
    /** Cuándo se abrió el panel (entra en ENTRADA ms). */
    private long abiertoEn;
    /** La pista «Pulsa O» sale una vez, unos segundos después de entrar a un mundo. */
    private long enMundoDesde;
    private boolean pistaHecha;

    private Juego(Plataforma plataforma) {
        this.plataforma = plataforma;
        if (PRUEBA) estado = ejemplo();
        capturas = new Capturas(plataforma.carpeta());
        panel = new Panel(this);
        puente = Puente.desdePropiedades(new Puente.Eventos() {
            public void evento(String tipo, JsonObject datos) {
                recibir(tipo, datos);
            }

            public void conexion(boolean conectado) {
                panel.marcar();
            }
        });
    }

    public static synchronized void iniciar(Plataforma plataforma) {
        if (actual == null) actual = new Juego(plataforma);
    }

    /** Null si todavía no se inició (la versión del juego lo hace al cargar el mod). */
    public static Juego actual() {
        return actual;
    }

    // ================= Llamadas de la versión del juego

    public void tick() {
        if (puente != null) puente.atender();
        avisos.tick();
        java.io.File nueva = capturas.revisar();
        if (nueva != null) aviso(T.t("Captura guardada", "Screenshot saved"), nueva.getName(), plataforma.tecla() + T.t(" › Capturas para mandarla", " › Screenshots to send it"), false);
    }

    /** La tecla de Parabellum: abre o cierra el panel. */
    public void tecla() {
        if (plataforma.panelAbierto()) plataforma.cerrarPanel();
        else plataforma.abrirPanel();
    }

    public void panelAbierto() {
        abiertoEn = System.currentTimeMillis();
        pistaHecha = true;
        marcarVisto();
        panel.abierto();
    }

    /** Lo que tarda el panel en aparecer (ms). -Dparabellum.prueba.lento=N: N veces más (para verlo en una captura). */
    private static final double ENTRADA = 180 * Math.max(1, Double.parseDouble(System.getProperty("parabellum.prueba.lento", "1")));

    /** De 0 a 1 mientras el panel aparece (se dibuja con esa transparencia). */
    public float panelAlfa() {
        return (float) entrada();
    }

    /** Cuánto más abajo va el panel mientras aparece, en píxeles reales (múltiplo de la escala). */
    public int panelDesliz(int escala) {
        return (int) Math.round((1 - entrada()) * 10) * Math.max(1, escala);
    }

    /** Avance de la entrada del panel, con salida suave (rápido al principio y frena al final). */
    private double entrada() {
        double t = Math.min(1, (System.currentTimeMillis() - abiertoEn) / ENTRADA);
        return 1 - Math.pow(1 - t, 3);
    }

    public void panelCerrado() {
        panel.cerrado();
    }

    /** Imagen del panel (en píxeles reales) y su esquina. */
    public Imagen dibujarPanel(int anchoVentana, int altoVentana, int escala) {
        return panel.dibujar(anchoVentana, altoVentana, escala);
    }

    public int panelX() {
        return panel.x();
    }

    public int panelY() {
        return panel.y();
    }

    /** Coordenadas en píxeles reales de la ventana. */
    public void raton(double x, double y) {
        panel.raton(x, y);
    }

    public boolean clic(double x, double y, int boton) {
        return panel.clic(x, y, boton);
    }

    public boolean rueda(double x, double y, double cantidad) {
        return panel.rueda(x, y, cantidad);
    }

    /** True si el panel usó la tecla (si no, Esc cierra). */
    public boolean teclaPanel(int tecla, int mods) {
        return panel.tecla(tecla, mods);
    }

    public boolean caracter(char c) {
        return panel.caracter(c);
    }

    /** Los avisos (null si no hay); van abajo a la derecha (ver avisosX/avisosY). Solo se piden con un mundo abierto. */
    public Imagen avisos(int anchoVentana, int altoVentana, int escala) {
        pista();
        return avisos.imagen(anchoVentana, altoVentana, escala);
    }

    /** Dónde dibujar los avisos, en píxeles reales. */
    public int avisosX(int anchoVentana, int escala) {
        return avisos.x(anchoVentana, escala);
    }

    public int avisosY(int anchoVentana, int altoVentana, int escala) {
        return avisos.y(anchoVentana, altoVentana, escala);
    }

    // ================= Para el núcleo

    /** -Dparabellum.prueba=true: el panel se dibuja como conectado (para probar el diseño sin Parabellum). */
    private static final boolean PRUEBA = Boolean.getBoolean("parabellum.prueba");

    /** Datos de ejemplo para el modo de prueba (un amigo y una conversación). */
    @SuppressWarnings("deprecation")
    private static JsonObject ejemplo() {
        return new com.google.gson.JsonParser().parse("{\"ev\":\"estado\",\"sesion\":true,\"yo\":{\"nombre\":\"Prueba\"},\"puedoInvitar\":true,"
            + "\"amigos\":[{\"uid\":\"a\",\"nombre\":\"Amigo\",\"enLinea\":true,\"jugando\":true,\"mundo\":true,\"pack\":\"Prueba\",\"unirse\":true}],"
            + "\"solicitudes\":[],\"servidores\":[],\"chats\":[{\"id\":\"c\",\"nombre\":\"Amigo\",\"tipo\":\"directo\",\"ultimo\":\"Hola\",\"sinLeer\":true}]}").getAsJsonObject();
    }

    boolean conectado() {
        return PRUEBA || (puente != null && puente.conectado());
    }

    boolean puenteDisponible() {
        return puente != null;
    }

    void puente(String op, JsonObject datos, Puente.Respuesta respuesta) {
        if (puente == null) {
            if (respuesta != null) respuesta.listo(null, T.t("Abre este juego desde Parabellum.", "Open this game from Parabellum."));
            return;
        }
        puente.pedir(op, datos, respuesta);
    }

    /** La primera vez que se entra a un mundo, si nunca se ha abierto el panel: cómo abrirlo. */
    private void pista() {
        if (pistaHecha) return;
        long ahora = System.currentTimeMillis();
        if (enMundoDesde == 0) enMundoDesde = ahora;
        if (ahora - enMundoDesde < 4000) return;
        pistaHecha = true;
        if (yaVisto()) return;
        avisos.agregar("Parabellum", T.t("Tus amigos, mensajes y capturas aquí mismo", "Your friends, messages and screenshots right here"),
            T.t("Pulsa ", "Press ") + plataforma.tecla() + T.t(" para abrirlo", " to open it"), true, 10000);
    }

    /** Marca en los datos de Parabellum: con abrir el panel una vez, ya no sale la pista en ninguna instancia. */
    private static java.io.File marca() {
        String fuentes = System.getProperty("parabellum.fuentes");
        return fuentes == null ? null : new java.io.File(new java.io.File(fuentes).getParentFile(), "panel-visto.txt");
    }

    /** Sin Parabellum (sin la carpeta) no hay a quién explicarle nada. */
    private static boolean yaVisto() {
        java.io.File f = marca();
        return f == null || f.exists();
    }

    private static void marcarVisto() {
        java.io.File f = marca();
        if (f == null || f.exists()) return;
        try {
            f.createNewFile();
        } catch (java.io.IOException e) {
            // Sin marca, la pista vuelve a salir la próxima vez: no pasa nada.
        }
    }

    void aviso(String titulo, String texto) {
        aviso(titulo, texto, null, false);
    }

    void aviso(String titulo, String texto, String pista, boolean destacado) {
        avisos.agregar(titulo, texto, pista, destacado);
    }

    private void recibir(String tipo, JsonObject datos) {
        if ("estado".equals(tipo)) {
            estado = datos;
            panel.marcar();
        } else if ("mensajes".equals(tipo)) {
            JsonArray lista = datos.has("mensajes") && datos.get("mensajes").isJsonArray() ? datos.getAsJsonArray("mensajes") : new JsonArray();
            panel.mensajes(Panel.texto(datos, "chat"), lista);
        } else if ("aviso".equals(tipo)) {
            String chat = Panel.texto(datos, "chat");
            // Si ya ves esa conversación, no hace falta avisar.
            if (chat != null && panel.viendo(chat)) return;
            String clase = Panel.texto(datos, "tipo");
            boolean invitacion = "invitacion".equals(clase);
            String texto = Panel.vistaPrevia(Panel.texto(datos, "texto"));
            String pista = plataforma.tecla() + (invitacion ? T.t(" › Mensajes para unirte", " › Messages to join") : "solicitud".equals(clase) ? T.t(" › Amigos", " › Friends") : chat != null ? T.t(" › Mensajes", " › Messages") : "");
            aviso(Panel.texto(datos, "titulo"), texto, pista, invitacion);
        }
    }
}
