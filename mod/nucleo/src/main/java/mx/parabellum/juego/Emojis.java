package mx.parabellum.juego;

import java.awt.BasicStroke;
import java.awt.Color;
import java.awt.geom.Arc2D;
import java.awt.geom.Ellipse2D;
import java.awt.geom.GeneralPath;
import java.awt.geom.RoundRectangle2D;

/**
 * Las reacciones del chat (las mismas seis que en Parabellum: 👍 ❤️ 😂 😮 😢 🔥) dibujadas con Java2D:
 * Minecraft no trae una fuente con emojis de color y así se ven igual en todas las versiones.
 */
final class Emojis {
    static final String[] REACCIONES = {"👍", "❤️", "😂", "😮", "😢", "🔥"};

    private static final Color AMARILLO = new Color(0xffc83d);
    private static final Color AMARILLO_OSCURO = new Color(0xe0a01a);
    private static final Color CAFE = new Color(0x5b3a1a);
    private static final Color ROJO = new Color(0xf0475a);
    private static final Color AZUL = new Color(0x5ab8f0);
    private static final Color NARANJA = new Color(0xff7a1a);

    private Emojis() {
    }

    /** True si es una de las seis (las demás se escriben con la fuente). */
    static boolean conocido(String emoji) {
        for (String r : REACCIONES) if (r.equals(emoji)) return true;
        return false;
    }

    /** Dibuja el emoji en un cuadro de `t` unidades con esquina en (x, y). */
    static void dibujar(Lienzo l, String emoji, double x, double y, double t) {
        double u = l.u;
        double px = x * u, py = y * u, s = t * u;
        if ("👍".equals(emoji)) pulgar(l, px, py, s);
        else if ("❤️".equals(emoji) || "❤".equals(emoji)) corazon(l, px, py, s);
        else if ("😂".equals(emoji)) cara(l, px, py, s, 0);
        else if ("😮".equals(emoji)) cara(l, px, py, s, 1);
        else if ("😢".equals(emoji)) cara(l, px, py, s, 2);
        else if ("🔥".equals(emoji)) fuego(l, px, py, s);
        else l.texto(emoji, x, y, t * 0.8, false, Lienzo.TEXTO);
    }

    private static void pulgar(Lienzo l, double x, double y, double s) {
        // Puño (con los dedos) y el pulgar hacia arriba.
        l.g.setColor(AMARILLO);
        l.g.fill(new RoundRectangle2D.Double(x + s * 0.30, y + s * 0.42, s * 0.56, s * 0.46, s * 0.16, s * 0.16));
        GeneralPath pulgar = new GeneralPath();
        pulgar.moveTo(x + s * 0.30, y + s * 0.50);
        pulgar.lineTo(x + s * 0.44, y + s * 0.20);
        pulgar.curveTo(x + s * 0.47, y + s * 0.10, x + s * 0.62, y + s * 0.10, x + s * 0.60, y + s * 0.24);
        pulgar.lineTo(x + s * 0.56, y + s * 0.44);
        pulgar.closePath();
        l.g.fill(pulgar);
        // Manga
        l.g.setColor(new Color(0x5b8def));
        l.g.fill(new RoundRectangle2D.Double(x + s * 0.12, y + s * 0.44, s * 0.18, s * 0.44, s * 0.06, s * 0.06));
        l.g.setColor(AMARILLO_OSCURO);
        l.g.setStroke(new BasicStroke((float) (s * 0.045), BasicStroke.CAP_ROUND, BasicStroke.JOIN_ROUND));
        for (int i = 1; i <= 3; i++) {
            double fy = y + s * (0.42 + i * 0.115);
            l.g.draw(new java.awt.geom.Line2D.Double(x + s * 0.62, fy, x + s * 0.84, fy));
        }
    }

    private static void corazon(Lienzo l, double x, double y, double s) {
        GeneralPath c = new GeneralPath();
        c.moveTo(x + s * 0.5, y + s * 0.88);
        c.curveTo(x + s * 0.08, y + s * 0.58, x + s * 0.02, y + s * 0.36, x + s * 0.12, y + s * 0.22);
        c.curveTo(x + s * 0.24, y + s * 0.06, x + s * 0.44, y + s * 0.10, x + s * 0.5, y + s * 0.26);
        c.curveTo(x + s * 0.56, y + s * 0.10, x + s * 0.76, y + s * 0.06, x + s * 0.88, y + s * 0.22);
        c.curveTo(x + s * 0.98, y + s * 0.36, x + s * 0.92, y + s * 0.58, x + s * 0.5, y + s * 0.88);
        c.closePath();
        l.g.setColor(ROJO);
        l.g.fill(c);
        l.g.setColor(new Color(255, 255, 255, 90));
        l.g.fill(new Ellipse2D.Double(x + s * 0.20, y + s * 0.22, s * 0.14, s * 0.10));
    }

    /** cual: 0 = riendo con lágrimas, 1 = sorprendido, 2 = triste. */
    private static void cara(Lienzo l, double x, double y, double s, int cual) {
        l.g.setColor(AMARILLO);
        l.g.fill(new Ellipse2D.Double(x + s * 0.06, y + s * 0.06, s * 0.88, s * 0.88));
        l.g.setColor(CAFE);
        l.g.setStroke(new BasicStroke((float) (s * 0.065), BasicStroke.CAP_ROUND, BasicStroke.JOIN_ROUND));
        if (cual == 0) {
            // Ojos cerrados de risa y boca abierta.
            l.g.draw(new Arc2D.Double(x + s * 0.24, y + s * 0.32, s * 0.18, s * 0.14, 0, 180, Arc2D.OPEN));
            l.g.draw(new Arc2D.Double(x + s * 0.58, y + s * 0.32, s * 0.18, s * 0.14, 0, 180, Arc2D.OPEN));
            l.g.fill(new Arc2D.Double(x + s * 0.24, y + s * 0.38, s * 0.52, s * 0.42, 180, 180, Arc2D.CHORD));
            l.g.setColor(AZUL);
            l.g.fill(new Ellipse2D.Double(x + s * 0.02, y + s * 0.44, s * 0.16, s * 0.24));
            l.g.fill(new Ellipse2D.Double(x + s * 0.82, y + s * 0.44, s * 0.16, s * 0.24));
        } else if (cual == 1) {
            l.g.fill(new Ellipse2D.Double(x + s * 0.30, y + s * 0.32, s * 0.11, s * 0.14));
            l.g.fill(new Ellipse2D.Double(x + s * 0.59, y + s * 0.32, s * 0.11, s * 0.14));
            l.g.fill(new Ellipse2D.Double(x + s * 0.39, y + s * 0.56, s * 0.22, s * 0.24));
        } else {
            l.g.fill(new Ellipse2D.Double(x + s * 0.30, y + s * 0.36, s * 0.10, s * 0.12));
            l.g.fill(new Ellipse2D.Double(x + s * 0.60, y + s * 0.36, s * 0.10, s * 0.12));
            l.g.draw(new Arc2D.Double(x + s * 0.32, y + s * 0.62, s * 0.36, s * 0.22, 20, 140, Arc2D.OPEN));
            l.g.setColor(AZUL);
            GeneralPath lagrima = new GeneralPath();
            lagrima.moveTo(x + s * 0.28, y + s * 0.50);
            lagrima.curveTo(x + s * 0.20, y + s * 0.64, x + s * 0.20, y + s * 0.74, x + s * 0.28, y + s * 0.74);
            lagrima.curveTo(x + s * 0.36, y + s * 0.74, x + s * 0.36, y + s * 0.64, x + s * 0.28, y + s * 0.50);
            l.g.fill(lagrima);
        }
    }

    private static void fuego(Lienzo l, double x, double y, double s) {
        GeneralPath fuera = new GeneralPath();
        fuera.moveTo(x + s * 0.50, y + s * 0.92);
        fuera.curveTo(x + s * 0.20, y + s * 0.92, x + s * 0.12, y + s * 0.66, x + s * 0.20, y + s * 0.48);
        fuera.curveTo(x + s * 0.26, y + s * 0.36, x + s * 0.34, y + s * 0.34, x + s * 0.34, y + s * 0.20);
        fuera.curveTo(x + s * 0.46, y + s * 0.28, x + s * 0.48, y + s * 0.40, x + s * 0.46, y + s * 0.46);
        fuera.curveTo(x + s * 0.56, y + s * 0.36, x + s * 0.56, y + s * 0.20, x + s * 0.50, y + s * 0.06);
        fuera.curveTo(x + s * 0.74, y + s * 0.20, x + s * 0.90, y + s * 0.46, x + s * 0.84, y + s * 0.66);
        fuera.curveTo(x + s * 0.80, y + s * 0.84, x + s * 0.68, y + s * 0.92, x + s * 0.50, y + s * 0.92);
        fuera.closePath();
        l.g.setColor(NARANJA);
        l.g.fill(fuera);
        GeneralPath dentro = new GeneralPath();
        dentro.moveTo(x + s * 0.50, y + s * 0.90);
        dentro.curveTo(x + s * 0.34, y + s * 0.90, x + s * 0.30, y + s * 0.74, x + s * 0.38, y + s * 0.62);
        dentro.curveTo(x + s * 0.44, y + s * 0.54, x + s * 0.48, y + s * 0.50, x + s * 0.50, y + s * 0.42);
        dentro.curveTo(x + s * 0.62, y + s * 0.54, x + s * 0.70, y + s * 0.66, x + s * 0.66, y + s * 0.78);
        dentro.curveTo(x + s * 0.62, y + s * 0.88, x + s * 0.56, y + s * 0.90, x + s * 0.50, y + s * 0.90);
        dentro.closePath();
        l.g.setColor(AMARILLO);
        l.g.fill(dentro);
    }
}
