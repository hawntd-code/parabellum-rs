package mx.parabellum.juego;

/** Una caja de texto de una línea (agregar amigo, escribir un mensaje). */
final class Campo {
    // Teclas de GLFW (las mismas en todas las versiones de Minecraft).
    static final int ENTER = 257, ENTER_NUMERICO = 335, BORRAR = 259, SUPR = 261, DERECHA = 262, IZQUIERDA = 263,
            INICIO = 268, FIN = 269, ESCAPE = 256, TECLA_V = 86, TECLA_A = 65, TECLA_C = 67;
    static final int CTRL = 2;

    final String marcador;
    final int maximo;
    String valor = "";
    int cursor;
    boolean foco;
    /** Todo seleccionado (Ctrl+A): lo siguiente que se escriba lo reemplaza. */
    boolean todo;

    Campo(String marcador, int maximo) {
        this.marcador = marcador;
        this.maximo = maximo;
    }

    void poner(String texto) {
        valor = texto == null ? "" : texto;
        cursor = valor.length();
        todo = false;
    }

    private void insertar(String texto) {
        if (todo) {
            valor = "";
            cursor = 0;
            todo = false;
        }
        texto = texto.replace("\r", "").replace("\n", " ");
        int cabe = maximo - valor.length();
        if (cabe <= 0) return;
        if (texto.length() > cabe) texto = texto.substring(0, cabe);
        valor = valor.substring(0, cursor) + texto + valor.substring(cursor);
        cursor += texto.length();
    }

    boolean caracter(char c) {
        if (c < 32 || c == 127) return false;
        insertar(String.valueOf(c));
        return true;
    }

    /** Devuelve true si la tecla era para la caja. */
    boolean tecla(int tecla, int mods, Plataforma plataforma) {
        boolean ctrl = (mods & CTRL) != 0;
        if (ctrl && tecla == TECLA_V) {
            String pegado = plataforma.portapapeles();
            if (pegado != null) insertar(pegado);
            return true;
        }
        if (ctrl && tecla == TECLA_A) {
            todo = valor.length() > 0;
            cursor = valor.length();
            return true;
        }
        if (ctrl && tecla == TECLA_C) {
            if (todo) plataforma.copiar(valor);
            return true;
        }
        switch (tecla) {
            case BORRAR:
                if (todo) {
                    poner("");
                } else if (cursor > 0) {
                    int desde = cursor - 1;
                    if (ctrl) {
                        while (desde > 0 && valor.charAt(desde - 1) == ' ') desde--;
                        while (desde > 0 && valor.charAt(desde - 1) != ' ') desde--;
                    }
                    valor = valor.substring(0, desde) + valor.substring(cursor);
                    cursor = desde;
                }
                return true;
            case SUPR:
                if (todo) poner("");
                else if (cursor < valor.length()) valor = valor.substring(0, cursor) + valor.substring(cursor + 1);
                return true;
            case IZQUIERDA:
                todo = false;
                if (cursor > 0) cursor--;
                return true;
            case DERECHA:
                todo = false;
                if (cursor < valor.length()) cursor++;
                return true;
            case INICIO:
                todo = false;
                cursor = 0;
                return true;
            case FIN:
                todo = false;
                cursor = valor.length();
                return true;
            default:
                return false;
        }
    }

    /** Dibuja la caja; devuelve el ancho usado. */
    void dibujar(Lienzo l, double x, double y, double w, double h, boolean parpadeo) {
        l.rect(x, y, w, h, 8, Lienzo.CAMPO);
        l.borde(x, y, w, h, 8, foco ? Lienzo.ACENTO : Lienzo.BORDE);
        double tamano = 13;
        double ty = y + (h - l.altoLinea(tamano)) / 2;
        java.awt.Shape antes = l.g.getClip();
        l.g.clip(l.forma(x + 10, y, w - 20, h, 0));
        if (valor.isEmpty()) {
            l.texto(marcador, x + 12, ty, tamano, false, Lienzo.TENUE);
            if (foco && parpadeo) l.rect(x + 12, ty + 1, 1.5, l.altoLinea(tamano) - 2, 0, Lienzo.TEXTO);
        } else {
            // Si no cabe, se ve el final (donde está el cursor).
            double antesCursor = l.ancho(valor.substring(0, cursor), tamano, false);
            double desplazar = Math.max(0, antesCursor - (w - 28));
            if (todo) l.rect(x + 12 - desplazar, ty, l.ancho(valor, tamano, false), l.altoLinea(tamano), 2, Lienzo.ACENTO_SUAVE);
            l.texto(valor, x + 12 - desplazar, ty, tamano, false, Lienzo.TEXTO);
            if (foco && parpadeo) l.rect(x + 12 - desplazar + antesCursor, ty + 1, 1.5, l.altoLinea(tamano) - 2, 0, Lienzo.TEXTO);
        }
        l.g.setClip(antes);
    }
}
