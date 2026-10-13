package mx.parabellum.juego;

import java.awt.BasicStroke;
import java.awt.Color;
import java.awt.Shape;
import java.awt.geom.AffineTransform;
import java.awt.geom.Ellipse2D;
import java.awt.geom.Path2D;
import java.awt.geom.RoundRectangle2D;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Los iconos de Parabellum (los mismos trazos de ui/index.html, en una caja de 24×24 y con línea de 2,
 * como en la ventana), dibujados con Java2D. Cada icono es una lista de formas: «M…» es un trazo de SVG,
 * «c x y r» un círculo, «f x y r» un círculo relleno y «r x y w h radio» un rectángulo.
 */
final class Iconos {
    private static final Map<String, String[]> TRAZOS = new HashMap<String, String[]>();
    private static final Map<String, List<Object[]>> HECHOS = new HashMap<String, List<Object[]>>();

    static {
        TRAZOS.put("users", new String[] {"c 9 8.5 3.5", "M2.5 19.5a6.5 6.5 0 0 1 13 0", "M15.5 5.2a3.5 3.5 0 0 1 0 6.6M18 14.2a6.5 6.5 0 0 1 3.5 5.3"});
        TRAZOS.put("chat", new String[] {"M21 12a8 8 0 0 1-11.6 7.1L4 20.5l1.4-4.9A8 8 0 1 1 21 12z"});
        TRAZOS.put("image", new String[] {"r 3.5 4.5 17 15 2.5", "c 9 10 1.8", "m20.5 16-5-5L6 19.5"});
        TRAZOS.put("user-plus", new String[] {"c 10 8 4", "M3 20.5a7 7 0 0 1 12.2-4.7M18.5 13v6M15.5 16h6"});
        TRAZOS.put("plus", new String[] {"M12 5v14M5 12h14"});
        TRAZOS.put("send", new String[] {"M21 3 10.5 13.5M21 3l-6.5 18-4-7.5L3 9.5z"});
        TRAZOS.put("smile", new String[] {"c 12 12 8.5", "M8.5 14.5a4.5 4.5 0 0 0 7 0", "f 9 9.5 1.1", "f 15 9.5 1.1"});
        TRAZOS.put("reply", new String[] {"M9.5 6 4 11.5 9.5 17", "M4 11.5h10a6 6 0 0 1 6 6V19"});
        TRAZOS.put("more", new String[] {"f 12 5.5 1.4", "f 12 12 1.4", "f 12 18.5 1.4"});
        TRAZOS.put("close", new String[] {"M6 6l12 12M18 6 6 18"});
        TRAZOS.put("search", new String[] {"c 11 11 7", "m20 20-3.6-3.6"});
        TRAZOS.put("play", new String[] {"M7 4.8v14.4a1 1 0 0 0 1.5.9l11.5-7.2a1 1 0 0 0 0-1.8L8.5 3.9A1 1 0 0 0 7 4.8z"});
        TRAZOS.put("star", new String[] {"M12 3.6l2.6 5.3 5.8.8-4.2 4.1 1 5.8-5.2-2.7-5.2 2.7 1-5.8-4.2-4.1 5.8-.8z"});
        TRAZOS.put("crown", new String[] {"M4 17.5 3 7.5l5 4 4-6 4 6 5-4-1 10z", "M4.5 20.5h15"});
        TRAZOS.put("folder", new String[] {"M3.5 7.5a2 2 0 0 1 2-2h4l2 2.5h7a2 2 0 0 1 2 2v7.5a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z"});
        TRAZOS.put("check", new String[] {"m5 12.5 4.5 4.5L19 7.5"});
        TRAZOS.put("trash", new String[] {"M4.5 7h15M10 11v6M14 11v6M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12M9 7V4.5h6V7"});
        TRAZOS.put("pencil", new String[] {"M4 20h4L19 9a2.8 2.8 0 0 0-4-4L4 16v4zM13.5 6.5l4 4"});
        TRAZOS.put("left", new String[] {"M15 5l-7 7 7 7"});
        TRAZOS.put("user", new String[] {"c 12 8 4", "M4.5 20.5a7.5 7.5 0 0 1 15 0"});
        TRAZOS.put("copy", new String[] {"r 8.5 8.5 11 11 2", "M15.5 8.5V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v7.5a2 2 0 0 0 2 2h2.5"});
        TRAZOS.put("server", new String[] {"r 3.5 4 17 7 2", "r 3.5 13 17 7 2", "f 7.5 7.5 1", "f 7.5 16.5 1"});
        TRAZOS.put("globe", new String[] {"c 12 12 9", "M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"});
        TRAZOS.put("camera", new String[] {"M4 8.5A2 2 0 0 1 6 6.5h2l1.5-2h5l1.5 2h2a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2z", "c 12 13 3.5"});
        TRAZOS.put("alert", new String[] {"c 12 12 9", "M12 7.5v5.5", "f 12 16.5 1.1"});
        TRAZOS.put("logout", new String[] {"M9 20H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h3M16 16l4-4-4-4M20 12H9"});
    }

    private Iconos() {
    }

    /** Dibuja el icono en un cuadro de `tamano` unidades con esquina en (x, y). */
    static void dibujar(Lienzo l, String nombre, double x, double y, double tamano, Color color) {
        List<Object[]> formas = formas(nombre);
        if (formas == null) return;
        double escala = tamano / 24 * l.u;
        AffineTransform t = new AffineTransform(escala, 0, 0, escala, x * l.u, y * l.u);
        l.g.setColor(color);
        l.g.setStroke(new BasicStroke((float) (2 * escala), BasicStroke.CAP_ROUND, BasicStroke.JOIN_ROUND));
        for (Object[] f : formas) {
            Shape forma = t.createTransformedShape((Shape) f[1]);
            if ((Boolean) f[0]) l.g.fill(forma);
            else l.g.draw(forma);
        }
    }

    private static synchronized List<Object[]> formas(String nombre) {
        List<Object[]> hechas = HECHOS.get(nombre);
        if (hechas != null) return hechas;
        String[] trazos = TRAZOS.get(nombre);
        if (trazos == null) return null;
        hechas = new ArrayList<Object[]>();
        for (String t : trazos) {
            String[] p = t.split(" ");
            if (t.startsWith("c ") || t.startsWith("f ")) {
                double cx = Double.parseDouble(p[1]), cy = Double.parseDouble(p[2]), r = Double.parseDouble(p[3]);
                hechas.add(new Object[] {t.startsWith("f "), new Ellipse2D.Double(cx - r, cy - r, r * 2, r * 2)});
            } else if (t.startsWith("r ")) {
                double rx = Double.parseDouble(p[1]), ry = Double.parseDouble(p[2]), rw = Double.parseDouble(p[3]), rh = Double.parseDouble(p[4]), rr = Double.parseDouble(p[5]);
                hechas.add(new Object[] {false, new RoundRectangle2D.Double(rx, ry, rw, rh, rr * 2, rr * 2)});
            } else {
                hechas.add(new Object[] {false, trazo(t)});
            }
        }
        HECHOS.put(nombre, hechas);
        return hechas;
    }

    // ================= Trazos de SVG (M L H V C S Q A Z, también en minúscula)

    private static Path2D trazo(String d) {
        Path2D.Double p = new Path2D.Double();
        List<String> tokens = tokens(d);
        int i = 0;
        char cmd = 'M';
        double cx = 0, cy = 0, inicioX = 0, inicioY = 0, ctrlX = 0, ctrlY = 0;
        while (i < tokens.size()) {
            String tk = tokens.get(i);
            if (Character.isLetter(tk.charAt(0))) {
                cmd = tk.charAt(0);
                i++;
                if (cmd == 'Z' || cmd == 'z') {
                    p.closePath();
                    cx = inicioX;
                    cy = inicioY;
                    continue;
                }
            }
            boolean rel = Character.isLowerCase(cmd);
            double ox = rel ? cx : 0, oy = rel ? cy : 0;
            switch (Character.toUpperCase(cmd)) {
                case 'M': {
                    cx = ox + num(tokens, i++);
                    cy = oy + num(tokens, i++);
                    p.moveTo(cx, cy);
                    inicioX = cx;
                    inicioY = cy;
                    cmd = rel ? 'l' : 'L'; // los pares que siguen son líneas
                    break;
                }
                case 'L':
                    cx = ox + num(tokens, i++);
                    cy = oy + num(tokens, i++);
                    p.lineTo(cx, cy);
                    break;
                case 'H':
                    cx = (rel ? cx : 0) + num(tokens, i++);
                    p.lineTo(cx, cy);
                    break;
                case 'V':
                    cy = (rel ? cy : 0) + num(tokens, i++);
                    p.lineTo(cx, cy);
                    break;
                case 'C': {
                    double x1 = ox + num(tokens, i++), y1 = oy + num(tokens, i++);
                    ctrlX = ox + num(tokens, i++);
                    ctrlY = oy + num(tokens, i++);
                    cx = ox + num(tokens, i++);
                    cy = oy + num(tokens, i++);
                    p.curveTo(x1, y1, ctrlX, ctrlY, cx, cy);
                    break;
                }
                case 'S': {
                    double x1 = 2 * cx - ctrlX, y1 = 2 * cy - ctrlY;
                    ctrlX = ox + num(tokens, i++);
                    ctrlY = oy + num(tokens, i++);
                    cx = ox + num(tokens, i++);
                    cy = oy + num(tokens, i++);
                    p.curveTo(x1, y1, ctrlX, ctrlY, cx, cy);
                    break;
                }
                case 'Q': {
                    ctrlX = ox + num(tokens, i++);
                    ctrlY = oy + num(tokens, i++);
                    cx = ox + num(tokens, i++);
                    cy = oy + num(tokens, i++);
                    p.quadTo(ctrlX, ctrlY, cx, cy);
                    break;
                }
                case 'A': {
                    double rx = num(tokens, i++), ry = num(tokens, i++), giro = num(tokens, i++);
                    boolean grande = num(tokens, i++) != 0, horario = num(tokens, i++) != 0;
                    double x = ox + num(tokens, i++), y = oy + num(tokens, i++);
                    arco(p, cx, cy, rx, ry, giro, grande, horario, x, y);
                    cx = x;
                    cy = y;
                    break;
                }
                default:
                    i++;
            }
        }
        return p;
    }

    private static double num(List<String> tokens, int i) {
        return i < tokens.size() ? Double.parseDouble(tokens.get(i)) : 0;
    }

    /** Separa letras y números ("a8 8 0 0 1-11.6 7.1", "5.8.8", "m20.5 16-5-5"). En los arcos, las
     * dos banderas pueden ir pegadas ("0 01"), pero en estos iconos siempre van separadas. */
    private static List<String> tokens(String d) {
        List<String> salida = new ArrayList<String>();
        StringBuilder actual = new StringBuilder();
        boolean punto = false;
        for (int i = 0; i < d.length(); i++) {
            char c = d.charAt(i);
            if (Character.isLetter(c) && c != 'e') {
                if (actual.length() > 0) salida.add(actual.toString());
                actual.setLength(0);
                punto = false;
                salida.add(String.valueOf(c));
            } else if (c == ' ' || c == ',') {
                if (actual.length() > 0) salida.add(actual.toString());
                actual.setLength(0);
                punto = false;
            } else if (c == '-') {
                if (actual.length() > 0) salida.add(actual.toString());
                actual.setLength(0);
                actual.append(c);
                punto = false;
            } else if (c == '.') {
                if (punto) {
                    salida.add(actual.toString());
                    actual.setLength(0);
                }
                actual.append(c);
                punto = true;
            } else {
                actual.append(c);
            }
        }
        if (actual.length() > 0) salida.add(actual.toString());
        return salida;
    }

    /** Arco de SVG (forma de extremos) como curvas de Bézier (SVG 1.1, apéndice F.6.5). */
    private static void arco(Path2D p, double x1, double y1, double rx, double ry, double giro, boolean grande, boolean horario, double x2, double y2) {
        if (rx == 0 || ry == 0) {
            p.lineTo(x2, y2);
            return;
        }
        rx = Math.abs(rx);
        ry = Math.abs(ry);
        double phi = Math.toRadians(giro), cos = Math.cos(phi), sin = Math.sin(phi);
        double dx = (x1 - x2) / 2, dy = (y1 - y2) / 2;
        double x1p = cos * dx + sin * dy, y1p = -sin * dx + cos * dy;
        double lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
        if (lambda > 1) {
            rx *= Math.sqrt(lambda);
            ry *= Math.sqrt(lambda);
        }
        double num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p;
        double den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
        double coef = (grande == horario ? -1 : 1) * Math.sqrt(Math.max(0, num / den));
        double cxp = coef * rx * y1p / ry, cyp = -coef * ry * x1p / rx;
        double centroX = cos * cxp - sin * cyp + (x1 + x2) / 2, centroY = sin * cxp + cos * cyp + (y1 + y2) / 2;
        double t1 = angulo(1, 0, (x1p - cxp) / rx, (y1p - cyp) / ry);
        double dt = angulo((x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry);
        if (!horario && dt > 0) dt -= 2 * Math.PI;
        else if (horario && dt < 0) dt += 2 * Math.PI;
        int partes = (int) Math.ceil(Math.abs(dt) / (Math.PI / 2));
        double paso = dt / partes, k = 4.0 / 3 * Math.tan(paso / 4);
        double t = t1;
        for (int i = 0; i < partes; i++) {
            double c1 = Math.cos(t), s1 = Math.sin(t), c2 = Math.cos(t + paso), s2 = Math.sin(t + paso);
            double[] a = punto(centroX, centroY, rx, ry, cos, sin, c1 - k * s1, s1 + k * c1);
            double[] b = punto(centroX, centroY, rx, ry, cos, sin, c2 + k * s2, s2 - k * c2);
            double[] e = punto(centroX, centroY, rx, ry, cos, sin, c2, s2);
            p.curveTo(a[0], a[1], b[0], b[1], e[0], e[1]);
            t += paso;
        }
    }

    private static double[] punto(double cx, double cy, double rx, double ry, double cos, double sin, double ux, double uy) {
        double x = rx * ux, y = ry * uy;
        return new double[] {cx + cos * x - sin * y, cy + sin * x + cos * y};
    }

    private static double angulo(double ux, double uy, double vx, double vy) {
        double a = Math.atan2(uy, ux), b = Math.atan2(vy, vx);
        double d = b - a;
        while (d > Math.PI) d -= 2 * Math.PI;
        while (d < -Math.PI) d += 2 * Math.PI;
        return d;
    }
}
