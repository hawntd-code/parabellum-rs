package mx.parabellum.juego;

import javax.imageio.ImageIO;
import java.awt.image.BufferedImage;
import java.io.File;

/**
 * Dibuja el panel del juego sin Minecraft (modo de prueba, -Dparabellum.prueba=true) y guarda capturas de
 * cada estado: para revisar el diseño rápido. Uso: Vista <carpeta del juego> <carpeta de salida>.
 */
public class Vista {
    static Juego j;
    static File salida;
    static final int ANCHO = 1600, ALTO = 1000, ESCALA = 2;

    public static void main(String[] args) throws Exception {
        final File juego = new File(args[0]);
        salida = new File(args[1]);
        salida.mkdirs();
        Plataforma p = new Plataforma() {
            public File carpeta() { return juego; }
            public int anchoVentana() { return ANCHO; }
            public int altoVentana() { return ALTO; }
            public int escala() { return ESCALA; }
            public void abrirPanel() { }
            public void cerrarPanel() { }
            public boolean panelAbierto() { return true; }
            public void conectar(String direccion) { }
            public void enJuego(Runnable tarea) { tarea.run(); }
            public String portapapeles() { return null; }
            public void copiar(String texto) { }
            public String descripcion() { return "Fabric 1.21.1"; }
            public String tecla() { return "O"; }
        };
        Juego.iniciar(p);
        j = Juego.actual();
        j.panelAbierto();
        j.tick();
        Thread.sleep(400);
        guardar("01-amigos");
        mover(500, 300);
        guardar("02-amigos-encima");
        clic(500, 300, 1);
        guardar("03-amigos-menu");
        clic(10, 10, 0);
        // Mensajes, conversación directa
        j.panel.irAChat("d_a");
        Thread.sleep(200);
        guardar("04-mensajes");
        // Grupo
        j.panel.irAChat("g");
        Thread.sleep(200);
        guardar("05-grupo");
        double[] encima = {Double.parseDouble(System.getProperty("vx", "560")), Double.parseDouble(System.getProperty("vy", "300"))};
        mover(encima[0], encima[1]);
        guardar("06-grupo-encima");
        clic(encima[0], encima[1], 1);
        guardar("07-menu-mensaje");
        j.panel.tecla(Campo.ESCAPE, 0);
        // Responder: la herramienta ↩ junto al mensaje de Dani
        mover(encima[0], encima[1]);
        guardar("06b");
        clic(788, 266, 0);
        j.panel.caracter('v');
        j.panel.caracter('a');
        guardar("11-respuesta");
        // Encima de un mensaje propio (editar y borrar)
        mover(800, 480);
        guardar("12-mio-encima");
        j.panel.tecla(Campo.ESCAPE, 0);
        String[] nombres = {"08-nuevo-grupo", "09-miembros"};
        // Nuevo grupo: botón de la lista de conversaciones
        clic(196 + 248 - 42 + 15, 31, 0);
        j.panel.caracter('L');
        j.panel.caracter('o');
        j.panel.caracter('s');
        guardar(nombres[0]);
        j.panel.tecla(Campo.ESCAPE, 0);
        // Miembros: clic en el avatar del grupo de la cabecera
        clic(196 + 248 + 16 + 17, 29, 0);
        guardar(nombres[1]);
        j.panel.tecla(Campo.ESCAPE, 0);
        // Capturas
        clic(60, 224, 0);
        j.tick();
        Thread.sleep(1700);
        j.tick();
        Thread.sleep(600);
        guardar("10-capturas");
        System.out.println("listo");
        System.exit(0);
    }

    static double u() {
        Imagen i = j.dibujarPanel(ANCHO, ALTO, ESCALA);
        return i.ancho / 880.0;
    }

    static void mover(double ux, double uy) {
        double u = u();
        j.raton(j.panelX() + ux * u, j.panelY() + uy * u);
    }

    static void clic(double ux, double uy, int boton) {
        double u = u();
        j.clic(j.panelX() + ux * u, j.panelY() + uy * u, boton);
    }

    static void guardar(String nombre) throws Exception {
        Thread.sleep(150);
        Imagen i = j.dibujarPanel(ANCHO, ALTO, ESCALA);
        BufferedImage copia = new BufferedImage(i.ancho, i.alto, BufferedImage.TYPE_INT_ARGB);
        copia.getGraphics().drawImage(i.buffer, 0, 0, null);
        ImageIO.write(copia, "png", new File(salida, nombre + ".png"));
    }
}
