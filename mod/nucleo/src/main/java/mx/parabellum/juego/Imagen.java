package mx.parabellum.juego;

import java.awt.Rectangle;
import java.awt.image.BufferedImage;
import java.awt.image.DataBufferInt;

/**
 * Un dibujo del núcleo (el panel o los avisos) que la versión del juego sube como textura. Recuerda qué
 * parte cambió desde la última subida, para no copiar la imagen entera en cada tecla.
 */
public final class Imagen {
    public final int ancho;
    public final int alto;
    /** Píxeles ARGB (sin premultiplicar), fila por fila. */
    public final int[] pixeles;
    final BufferedImage buffer;
    private final int[] anterior;
    private Rectangle sucio;
    /** Sube con cada cambio. */
    public int version;

    Imagen(int ancho, int alto) {
        this.ancho = Math.max(1, ancho);
        this.alto = Math.max(1, alto);
        buffer = new BufferedImage(this.ancho, this.alto, BufferedImage.TYPE_INT_ARGB);
        pixeles = ((DataBufferInt) buffer.getRaster().getDataBuffer()).getData();
        anterior = new int[pixeles.length];
        sucio = new Rectangle(0, 0, this.ancho, this.alto);
    }

    /** Tras dibujar: anota qué cambió respecto a lo anterior. */
    void terminado() {
        int x0 = ancho, y0 = alto, x1 = -1, y1 = -1;
        for (int y = 0; y < alto; y++) {
            int fila = y * ancho;
            for (int x = 0; x < ancho; x++) {
                int i = fila + x;
                if (pixeles[i] != anterior[i]) {
                    if (x < x0) x0 = x;
                    if (x > x1) x1 = x;
                    if (y < y0) y0 = y;
                    y1 = y;
                }
            }
        }
        if (x1 < 0) return;
        System.arraycopy(pixeles, 0, anterior, 0, pixeles.length);
        Rectangle cambio = new Rectangle(x0, y0, x1 - x0 + 1, y1 - y0 + 1);
        sucio = sucio == null ? cambio : sucio.union(cambio);
        version++;
    }

    /** La parte que hay que volver a subir (y la olvida). Null si no cambió nada. */
    public Rectangle tomarSucio() {
        Rectangle r = sucio;
        sucio = null;
        return r;
    }

    /** Píxel en formato ABGR (el de NativeImage.setPixelRGBA hasta la 1.21.1). */
    public int abgr(int x, int y) {
        int p = pixeles[y * ancho + x];
        return (p & 0xFF00FF00) | ((p & 0xFF) << 16) | ((p >> 16) & 0xFF);
    }

    /** Píxel en formato ARGB (el de NativeImage.setPixel desde la 1.21.2). */
    public int argb(int x, int y) {
        return pixeles[y * ancho + x];
    }
}
