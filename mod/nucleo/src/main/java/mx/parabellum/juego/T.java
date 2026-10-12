package mx.parabellum.juego;

/** Textos en español o inglés, según el idioma de Parabellum (-Dparabellum.idioma). */
final class T {
    private static final boolean INGLES = "en".equals(System.getProperty("parabellum.idioma"));

    private T() {
    }

    static String t(String es, String en) {
        return INGLES ? en : es;
    }
}
