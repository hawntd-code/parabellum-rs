package mx.parabellum.juego;

import java.awt.AlphaComposite;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;

/**
 * Avisos en la esquina de abajo a la derecha mientras juegas (mensajes, invitaciones, solicitudes); arriba
 * los taparían los del juego (logros, recetas, tutorial). El más nuevo va pegado a la esquina. Entran
 * deslizándose desde la orilla, se van igual y los demás se acomodan suave. Solo se vuelven a dibujar
 * mientras algo se mueve; quietos, no cuestan nada.
 */
final class Avisos {
    private static final long DURA = 6000;
    /** Lo que tardan en entrar y en irse (ms). */
    private static final double ENTRA = 260, SALE = 200;
    private static final int MAXIMO = 3;
    private static final double ANCHO = 300, ALTO = 62, SEPARACION = 8, DESLIZ = 48;

    private static final class Aviso {
        final String titulo, texto, pista;
        final boolean destacado;
        final long desde, hasta;
        /** Cuándo empezó a irse (0 mientras se ve). */
        long saliendo;
        /** Dónde se dibuja; va hacia su lugar cuando llega o se va otro. */
        double y = Double.NaN;

        Aviso(String titulo, String texto, String pista, boolean destacado, long dura) {
            this.titulo = titulo;
            this.texto = texto;
            this.pista = pista;
            this.destacado = destacado;
            desde = System.currentTimeMillis();
            hasta = desde + dura;
        }
    }

    private final List<Aviso> lista = new ArrayList<Aviso>();
    private Imagen imagen;
    private boolean cambio = true;
    private double u;
    private long ultimo;

    void agregar(String titulo, String texto, String pista, boolean destacado) {
        agregar(titulo, texto, pista, destacado, DURA);
    }

    void agregar(String titulo, String texto, String pista, boolean destacado, long dura) {
        lista.add(0, new Aviso(titulo, texto == null ? "" : texto, pista, destacado, dura));
        // Si ya hay muchos, el más viejo se va (con su salida).
        int visibles = 0;
        for (Aviso a : lista) if (a.saliendo == 0 && ++visibles > MAXIMO) a.saliendo = System.currentTimeMillis();
        cambio = true;
    }

    void tick() {
        long ahora = System.currentTimeMillis();
        for (Aviso a : lista) {
            if (a.saliendo == 0 && a.hasta < ahora) {
                a.saliendo = ahora;
                cambio = true;
            }
        }
    }

    /** La imagen de los avisos (null si no hay); `escala` alinea el tamaño a píxeles de la interfaz. */
    Imagen imagen(int anchoVentana, int altoVentana, int escala) {
        long ahora = System.currentTimeMillis();
        for (Iterator<Aviso> it = lista.iterator(); it.hasNext(); ) {
            Aviso a = it.next();
            if (a.saliendo != 0 && ahora - a.saliendo >= SALE) {
                it.remove();
                cambio = true;
            }
        }
        if (lista.isEmpty()) {
            ultimo = 0;
            return null;
        }
        double nuevoU = Math.max(0.7, Math.min(1.6, Math.min(anchoVentana / 1100.0, altoVentana / 680.0)));
        int ancho = alinear((int) Math.round(ANCHO * nuevoU), escala);
        // Siempre del mismo alto (cabe uno más mientras otro se va): así la textura no se rehace en cada cuadro.
        int alto = alinear((int) Math.round((ALTO * (MAXIMO + 1) + SEPARACION * MAXIMO) * nuevoU), escala);
        if (imagen == null || imagen.ancho != ancho || imagen.alto != alto || nuevoU != u) {
            imagen = new Imagen(ancho, alto);
            u = nuevoU;
            cambio = true;
        }
        double dt = ultimo == 0 ? 0 : Math.min(100, ahora - ultimo);
        ultimo = ahora;
        // Desde abajo: el más nuevo en el último lugar de la imagen y los anteriores encima.
        double lugar = ALTO * MAXIMO + SEPARACION * MAXIMO;
        for (Aviso a : lista) {
            if (Double.isNaN(a.y) || Math.abs(a.y - lugar) < 0.3) {
                a.y = lugar;
            } else {
                a.y += (lugar - a.y) * (1 - Math.exp(-dt / 70));
                cambio = true;
            }
            if (ahora - a.desde < ENTRA || a.saliendo != 0) cambio = true;
            lugar -= ALTO + SEPARACION;
        }
        if (cambio) {
            cambio = false;
            Lienzo l = new Lienzo(imagen.buffer, u);
            for (Aviso a : lista) dibujar(l, a, ahora);
            l.listo();
            imagen.terminado();
        }
        return imagen;
    }

    /** Entra desde la orilla aclarándose (frena al llegar) y se va igual hacia afuera (acelera). */
    private static void dibujar(Lienzo l, Aviso a, long ahora) {
        double alfa, x;
        if (a.saliendo != 0) {
            double t = Math.min(1, (ahora - a.saliendo) / SALE);
            alfa = 1 - t;
            x = t * t * t * DESLIZ;
        } else {
            double t = Math.min(1, (ahora - a.desde) / ENTRA);
            double k = 1 - Math.pow(1 - t, 3);
            alfa = k;
            x = (1 - k) * DESLIZ;
        }
        if (alfa <= 0.01) return;
        l.g.setComposite(AlphaComposite.getInstance(AlphaComposite.SRC_OVER, (float) alfa));
        double y = a.y;
        l.rect(x, y, ANCHO, ALTO, 10, new java.awt.Color(0x1e, 0x1d, 0x23, 240));
        l.borde(x, y, ANCHO, ALTO, 10, a.destacado ? Lienzo.ACENTO : Lienzo.BORDE);
        l.rect(x, y + 10, 3, ALTO - 20, 1.5, a.destacado ? Lienzo.ACENTO : Lienzo.ACENTO_TEXTO);
        l.texto(l.recortar(a.titulo, ANCHO - 28, 13, true), x + 14, y + 9, 13, true, Lienzo.TEXTO);
        l.texto(l.recortar(a.texto, ANCHO - 28, 12, false), x + 14, y + 26, 12, false, Lienzo.APAGADO);
        if (a.pista != null) l.texto(l.recortar(a.pista, ANCHO - 28, 11, false), x + 14, y + 43, 11, false, Lienzo.ACENTO_TEXTO);
        l.g.setComposite(AlphaComposite.SrcOver);
    }

    /** Dónde va la imagen, en píxeles reales (múltiplos de la escala): pegada a la derecha. */
    int x(int anchoVentana, int escala) {
        int ancho = imagen == null ? 0 : imagen.ancho;
        return alinear(anchoVentana - ancho - 8 * Math.max(1, escala), escala);
    }

    /** Pegada abajo; si en una ventana angosta chocaría con la barra de objetos, va encima de ella y de la vida. */
    int y(int anchoVentana, int altoVentana, int escala) {
        int s = Math.max(1, escala);
        int alto = imagen == null ? 0 : imagen.alto;
        // Borde derecho de la barra de objetos (91) y del espacio de la otra mano (29), en unidades de la interfaz.
        int barra = anchoVentana / 2 + 120 * s;
        int abajo = x(anchoVentana, escala) < barra ? 52 * s : 8 * s;
        return alinear(altoVentana - alto - abajo, escala);
    }

    static int alinear(int pixeles, int escala) {
        int e = Math.max(1, escala);
        return Math.max(e, (pixeles / e) * e);
    }
}
