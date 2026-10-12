package mx.parabellum.juego;

import javax.imageio.IIOImage;
import javax.imageio.ImageIO;
import javax.imageio.ImageWriteParam;
import javax.imageio.ImageWriter;
import javax.imageio.stream.ImageOutputStream;
import java.awt.Graphics2D;
import java.awt.RenderingHints;
import java.awt.image.BufferedImage;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.ThreadFactory;

/** Las capturas del juego (F2): las últimas, sus miniaturas y cómo se mandan al chat. */
final class Capturas {
    static final int MOSTRAR = 12;

    private final File carpeta;
    private final Map<String, BufferedImage> miniaturas = new ConcurrentHashMap<String, BufferedImage>();
    private final Map<String, Boolean> cargando = new ConcurrentHashMap<String, Boolean>();
    static final ExecutorService HILO = Executors.newSingleThreadExecutor(new ThreadFactory() {
        public Thread newThread(Runnable r) {
            Thread t = new Thread(r, "Parabellum (imágenes)");
            t.setDaemon(true);
            return t;
        }
    });
    private List<File> ultimas = Collections.emptyList();
    private long revisado;
    private String masNueva;

    Capturas(File carpetaJuego) {
        carpeta = new File(carpetaJuego, "screenshots");
    }

    /** Revisa la carpeta (cada pocos segundos). Devuelve la captura nueva, si la hay. */
    File revisar() {
        long ahora = System.currentTimeMillis();
        if (ahora - revisado < 1500) return null;
        revisado = ahora;
        File[] archivos = carpeta.listFiles();
        if (archivos == null) return null;
        List<File> lista = new ArrayList<File>();
        for (File f : archivos) {
            String nombre = f.getName().toLowerCase();
            if (f.isFile() && (nombre.endsWith(".png") || nombre.endsWith(".jpg"))) lista.add(f);
        }
        Collections.sort(lista, new Comparator<File>() {
            public int compare(File a, File b) {
                return Long.compare(b.lastModified(), a.lastModified());
            }
        });
        if (lista.size() > MOSTRAR) lista = new ArrayList<File>(lista.subList(0, MOSTRAR));
        ultimas = lista;
        String primera = lista.isEmpty() ? null : lista.get(0).getName();
        File nueva = null;
        if (masNueva != null && primera != null && !primera.equals(masNueva)) nueva = lista.get(0);
        if (primera != null) masNueva = primera;
        else masNueva = "";
        return nueva;
    }

    List<File> ultimas() {
        return ultimas;
    }

    /** La miniatura (se carga en segundo plano; null mientras tanto). */
    BufferedImage miniatura(final File archivo, final Runnable alCargar) {
        final String clave = archivo.getAbsolutePath();
        BufferedImage hecha = miniaturas.get(clave);
        if (hecha != null || cargando.containsKey(clave)) return hecha;
        cargando.put(clave, true);
        HILO.execute(new Runnable() {
            public void run() {
                try {
                    BufferedImage original = ImageIO.read(archivo);
                    if (original != null) {
                        miniaturas.put(clave, escalar(original, 480));
                        alCargar.run();
                    }
                } catch (Exception ignored) {
                }
            }
        });
        return null;
    }

    static BufferedImage escalar(BufferedImage original, int maximo) {
        double escala = Math.min(1.0, (double) maximo / Math.max(original.getWidth(), original.getHeight()));
        int w = Math.max(1, (int) Math.round(original.getWidth() * escala));
        int h = Math.max(1, (int) Math.round(original.getHeight() * escala));
        BufferedImage salida = new BufferedImage(w, h, BufferedImage.TYPE_INT_RGB);
        Graphics2D g = salida.createGraphics();
        g.setRenderingHint(RenderingHints.KEY_INTERPOLATION, RenderingHints.VALUE_INTERPOLATION_BILINEAR);
        g.setRenderingHint(RenderingHints.KEY_RENDERING, RenderingHints.VALUE_RENDER_QUALITY);
        g.drawImage(original, 0, 0, w, h, null);
        g.dispose();
        return salida;
    }

    /** Lo mismo que hace Parabellum con las imágenes del chat: máximo 1280 px y ~330 KB en JPEG. */
    static Object[] paraChat(File archivo) throws Exception {
        BufferedImage original = ImageIO.read(archivo);
        if (original == null) throw new Exception(T.t("No se pudo leer la captura.", "Could not read the screenshot."));
        double escala = 1.0;
        for (int intento = 0; intento < 8; intento++) {
            BufferedImage imagen = escalar(original, (int) Math.round(1280 * escala));
            for (float calidad : new float[] {0.86f, 0.78f, 0.68f}) {
                byte[] jpeg = jpeg(imagen, calidad);
                if (jpeg.length <= 330 * 1024) {
                    String datos = "data:image/jpeg;base64," + java.util.Base64.getEncoder().encodeToString(jpeg);
                    return new Object[] {datos, imagen.getWidth(), imagen.getHeight()};
                }
            }
            escala *= 0.8;
        }
        throw new Exception(T.t("La captura es muy pesada.", "The screenshot is too large."));
    }

    private static byte[] jpeg(BufferedImage imagen, float calidad) throws Exception {
        ImageWriter escritor = ImageIO.getImageWritersByFormatName("jpeg").next();
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        ImageOutputStream salida = ImageIO.createImageOutputStream(bytes);
        try {
            escritor.setOutput(salida);
            ImageWriteParam parametros = escritor.getDefaultWriteParam();
            parametros.setCompressionMode(ImageWriteParam.MODE_EXPLICIT);
            parametros.setCompressionQuality(calidad);
            escritor.write(null, new IIOImage(imagen, null, null), parametros);
        } finally {
            salida.close();
            escritor.dispose();
        }
        return bytes.toByteArray();
    }
}
