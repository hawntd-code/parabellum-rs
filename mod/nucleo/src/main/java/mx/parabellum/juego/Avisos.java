package mx.parabellum.juego;

import java.util.ArrayList;
import java.util.List;

/** Avisos en la esquina de arriba a la derecha mientras juegas (mensajes, invitaciones, solicitudes). */
final class Avisos {
    private static final long DURA = 6000;
    private static final int MAXIMO = 3;
    private static final double ANCHO = 300, ALTO = 62, SEPARACION = 8;

    private static final class Aviso {
        final String titulo, texto, pista;
        final boolean destacado;
        final long hasta;

        Aviso(String titulo, String texto, String pista, boolean destacado) {
            this.titulo = titulo;
            this.texto = texto;
            this.pista = pista;
            this.destacado = destacado;
            hasta = System.currentTimeMillis() + DURA;
        }
    }

    private final List<Aviso> lista = new ArrayList<Aviso>();
    private Imagen imagen;
    private boolean cambio = true;
    private double u;

    void agregar(String titulo, String texto, String pista, boolean destacado) {
        lista.add(0, new Aviso(titulo, texto == null ? "" : texto, pista, destacado));
        while (lista.size() > MAXIMO) lista.remove(lista.size() - 1);
        cambio = true;
    }

    void tick() {
        long ahora = System.currentTimeMillis();
        for (int i = lista.size() - 1; i >= 0; i--) {
            if (lista.get(i).hasta < ahora) {
                lista.remove(i);
                cambio = true;
            }
        }
    }

    /** La imagen de los avisos (null si no hay); `escala` alinea el tamaño a píxeles de la interfaz. */
    Imagen imagen(int anchoVentana, int altoVentana, int escala) {
        if (lista.isEmpty()) return null;
        double nuevoU = Math.max(0.7, Math.min(1.6, Math.min(anchoVentana / 1100.0, altoVentana / 680.0)));
        int ancho = alinear((int) Math.round(ANCHO * nuevoU), escala);
        int alto = alinear((int) Math.round((ALTO * lista.size() + SEPARACION * (lista.size() - 1)) * nuevoU), escala);
        if (imagen == null || imagen.ancho != ancho || imagen.alto != alto || nuevoU != u) {
            imagen = new Imagen(ancho, alto);
            u = nuevoU;
            cambio = true;
        }
        if (cambio) {
            cambio = false;
            Lienzo l = new Lienzo(imagen.buffer, u);
            double y = 0;
            for (Aviso a : lista) {
                l.rect(0, y, ANCHO, ALTO, 10, new java.awt.Color(0x1e, 0x1d, 0x23, 240));
                l.borde(0, y, ANCHO, ALTO, 10, a.destacado ? Lienzo.ACENTO : Lienzo.BORDE);
                l.rect(0, y + 10, 3, ALTO - 20, 1.5, a.destacado ? Lienzo.ACENTO : Lienzo.ACENTO_TEXTO);
                l.texto(l.recortar(a.titulo, ANCHO - 28, 13, true), 14, y + 9, 13, true, Lienzo.TEXTO);
                l.texto(l.recortar(a.texto, ANCHO - 28, 12, false), 14, y + 26, 12, false, Lienzo.APAGADO);
                if (a.pista != null) l.texto(l.recortar(a.pista, ANCHO - 28, 11, false), 14, y + 43, 11, false, Lienzo.ACENTO_TEXTO);
                y += ALTO + SEPARACION;
            }
            l.listo();
            imagen.terminado();
        }
        return imagen;
    }

    static int alinear(int pixeles, int escala) {
        int e = Math.max(1, escala);
        return Math.max(e, (pixeles / e) * e);
    }
}
