package mx.parabellum.juego;

import java.awt.AlphaComposite;
import java.awt.Color;
import java.awt.Font;
import java.awt.FontMetrics;
import java.awt.Graphics2D;
import java.awt.RenderingHints;
import java.awt.Shape;
import java.awt.geom.RoundRectangle2D;
import java.awt.image.BufferedImage;
import java.io.File;
import java.util.ArrayList;
import java.util.List;

/**
 * Dibujo con Java2D en unidades de diseño (u): todo se multiplica por la escala, así el panel se ve igual
 * de nítido en cualquier ventana. Colores y fuente (Inter) como en Parabellum.
 */
final class Lienzo {
    static final Color FONDO = new Color(0x1e1d23);
    static final Color LATERAL = new Color(0x17161b);
    static final Color TARJETA = new Color(0x2a2832);
    static final Color TARJETA_ENCIMA = new Color(0x34313d);
    static final Color CAMPO = new Color(0x141318);
    static final Color BORDE = new Color(0x3a3743);
    static final Color TEXTO = new Color(0xecebf1);
    static final Color APAGADO = new Color(0xa7a3b3);
    static final Color TENUE = new Color(0x7d798b);
    static final Color ACENTO = new Color(0x7f50f2);
    static final Color ACENTO_ENCIMA = new Color(0x9068f5);
    static final Color ACENTO_TEXTO = new Color(0xb79bff);
    static final Color ACENTO_SUAVE = new Color(139, 92, 246, 46);
    static final Color EXITO = new Color(0x3dd68c);
    static final Color PELIGRO = new Color(0xf0525d);

    private static Font regular;
    private static Font seminegrita;

    final Graphics2D g;
    final double u;

    Lienzo(BufferedImage imagen, double u) {
        this.u = u;
        g = imagen.createGraphics();
        g.setRenderingHint(RenderingHints.KEY_ANTIALIASING, RenderingHints.VALUE_ANTIALIAS_ON);
        g.setRenderingHint(RenderingHints.KEY_TEXT_ANTIALIASING, RenderingHints.VALUE_TEXT_ANTIALIAS_ON);
        g.setRenderingHint(RenderingHints.KEY_FRACTIONALMETRICS, RenderingHints.VALUE_FRACTIONALMETRICS_ON);
        g.setRenderingHint(RenderingHints.KEY_RENDERING, RenderingHints.VALUE_RENDER_QUALITY);
        g.setRenderingHint(RenderingHints.KEY_INTERPOLATION, RenderingHints.VALUE_INTERPOLATION_BILINEAR);
        g.setComposite(AlphaComposite.Src);
        g.setColor(new Color(0, 0, 0, 0));
        g.fillRect(0, 0, imagen.getWidth(), imagen.getHeight());
        g.setComposite(AlphaComposite.SrcOver);
    }

    void listo() {
        g.dispose();
    }

    int px(double unidades) {
        return (int) Math.round(unidades * u);
    }

    // ---------- Fuentes

    private static Font cargar(String archivo) {
        String carpeta = System.getProperty("parabellum.fuentes");
        if (carpeta != null) {
            try {
                return Font.createFont(Font.TRUETYPE_FONT, new File(carpeta, archivo));
            } catch (Exception ignored) {
                // Sin la fuente de Parabellum: la del sistema.
            }
        }
        return null;
    }

    static synchronized Font fuente(boolean negrita) {
        if (regular == null) {
            regular = cargar("Inter-Regular.ttf");
            seminegrita = cargar("Inter-SemiBold.ttf");
            if (regular == null) regular = new Font(Font.SANS_SERIF, Font.PLAIN, 12);
            if (seminegrita == null) seminegrita = regular.deriveFont(Font.BOLD);
        }
        return negrita ? seminegrita : regular;
    }

    Font fuente(double tamano, boolean negrita) {
        return fuente(negrita).deriveFont((float) (tamano * u));
    }

    // ---------- Formas

    void rect(double x, double y, double w, double h, double radio, Color color) {
        g.setColor(color);
        g.fill(forma(x, y, w, h, radio));
    }

    void borde(double x, double y, double w, double h, double radio, Color color) {
        g.setColor(color);
        g.setStroke(new java.awt.BasicStroke((float) Math.max(1, u)));
        g.draw(forma(x + 0.5 / u, y + 0.5 / u, w - 1 / u, h - 1 / u, radio));
    }

    Shape forma(double x, double y, double w, double h, double radio) {
        return new RoundRectangle2D.Double(x * u, y * u, w * u, h * u, radio * 2 * u, radio * 2 * u);
    }

    void circulo(double x, double y, double d, Color color) {
        g.setColor(color);
        g.fill(new java.awt.geom.Ellipse2D.Double(x * u, y * u, d * u, d * u));
    }

    // ---------- Texto

    /** Escribe en (x, y) = esquina superior izquierda de la línea. Devuelve el ancho en u. */
    double texto(String texto, double x, double y, double tamano, boolean negrita, Color color) {
        Font f = fuente(tamano, negrita);
        g.setFont(f);
        g.setColor(color);
        FontMetrics m = g.getFontMetrics();
        g.drawString(texto, (float) (x * u), (float) (y * u + m.getAscent()));
        return m.stringWidth(texto) / u;
    }

    /** Recorta con «…» para que quepa en el ancho. */
    String recortar(String texto, double ancho, double tamano, boolean negrita) {
        if (texto == null) return "";
        FontMetrics m = g.getFontMetrics(fuente(tamano, negrita));
        double max = ancho * u;
        if (m.stringWidth(texto) <= max) return texto;
        String corto = texto;
        while (corto.length() > 0 && m.stringWidth(corto + "…") > max) corto = corto.substring(0, corto.length() - 1);
        return corto + "…";
    }

    double ancho(String texto, double tamano, boolean negrita) {
        return g.getFontMetrics(fuente(tamano, negrita)).stringWidth(texto) / u;
    }

    double altoLinea(double tamano) {
        return g.getFontMetrics(fuente(tamano, false)).getHeight() / u;
    }

    /** Parte el texto en líneas que quepan en el ancho (respeta los saltos de línea). */
    List<String> lineas(String texto, double ancho, double tamano, boolean negrita) {
        List<String> salida = new ArrayList<String>();
        FontMetrics m = g.getFontMetrics(fuente(tamano, negrita));
        double max = ancho * u;
        for (String parrafo : (texto == null ? "" : texto).split("\n", -1)) {
            StringBuilder linea = new StringBuilder();
            for (String palabra : parrafo.split(" ", -1)) {
                String prueba = linea.length() == 0 ? palabra : linea + " " + palabra;
                if (m.stringWidth(prueba) <= max) {
                    linea.setLength(0);
                    linea.append(prueba);
                    continue;
                }
                if (linea.length() > 0) salida.add(linea.toString());
                linea.setLength(0);
                // Una palabra más larga que el renglón se parte a la fuerza.
                String resto = palabra;
                while (m.stringWidth(resto) > max && resto.length() > 1) {
                    int corte = resto.length();
                    while (corte > 1 && m.stringWidth(resto.substring(0, corte)) > max) corte--;
                    salida.add(resto.substring(0, corte));
                    resto = resto.substring(corte);
                }
                linea.append(resto);
            }
            salida.add(linea.toString());
        }
        return salida;
    }

    // ---------- Cabezas (las mismas que dibuja Parabellum a partir del nombre)

    static int hash(String texto) {
        int h = 0x811c9dc5;
        for (int i = 0; i < texto.length(); i++) {
            h ^= texto.charAt(i);
            h *= 16777619;
        }
        return h;
    }

    void cabeza(String nombre, double x, double y, double tamano) {
        long h = hash(nombre == null || nombre.isEmpty() ? "Jugador" : nombre) & 0xffffffffL;
        int[] pieles = {0xf2c7a5, 0xe0ac83, 0xc68a5e, 0xa8714a, 0x8d5a3b, 0x5c3a26};
        int[] pelos = {0x2b1d14, 0x4a2f1e, 0x7a4b2a, 0xc79a4a, 0x1d1a2b, 0x6b3fa0, 0xb5332e};
        int[] ojos = {0x3b82f6, 0x22a06b, 0x7c4a24, 0x8b5cf6};
        double p = tamano / 8;
        Shape antes = g.getClip();
        g.clip(forma(x, y, tamano, tamano, tamano * 0.18));
        cuadro(x, y, 0, 0, 8, 8, p, pieles[(int) (h % pieles.length)]);
        int pelo = pelos[(int) ((h >> 3) % pelos.length)];
        cuadro(x, y, 0, 0, 8, 2, p, pelo);
        cuadro(x, y, 0, 2, 1, ((h >> 9) % 2) == 1 ? 3 : 1, p, pelo);
        cuadro(x, y, 7, 2, 1, ((h >> 10) % 2) == 1 ? 3 : 1, p, pelo);
        cuadro(x, y, 1, 4, 2, 1, p, 0xffffff);
        cuadro(x, y, 5, 4, 2, 1, p, 0xffffff);
        int ojo = ojos[(int) ((h >> 6) % 4)];
        cuadro(x, y, 2, 4, 1, 1, p, ojo);
        cuadro(x, y, 5, 4, 1, 1, p, ojo);
        g.setColor(new Color(0, 0, 0, 72));
        g.fill(new java.awt.geom.Rectangle2D.Double((x + 3 * p) * u, (y + 6 * p) * u, 2 * p * u, p * u));
        g.setClip(antes);
    }

    private void cuadro(double x, double y, int cx, int cy, int w, int h, double p, int rgb) {
        g.setColor(new Color(rgb));
        g.fill(new java.awt.geom.Rectangle2D.Double((x + cx * p) * u, (y + cy * p) * u, w * p * u + 0.5, h * p * u + 0.5));
    }

    void imagen(BufferedImage imagen, double x, double y, double w, double h, double radio) {
        Shape antes = g.getClip();
        g.clip(forma(x, y, w, h, radio));
        g.drawImage(imagen, (int) Math.round(x * u), (int) Math.round(y * u), (int) Math.round(w * u), (int) Math.round(h * u), null);
        g.setClip(antes);
    }
}
