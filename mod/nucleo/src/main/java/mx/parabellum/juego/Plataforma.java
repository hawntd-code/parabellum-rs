package mx.parabellum.juego;

import java.io.File;

/**
 * Lo que cambia entre versiones de Minecraft y loaders. Cada carpeta de {@code versiones/} lo implementa
 * (dibujar las imágenes del panel, la tecla, entrar a un servidor); todo lo demás vive en este núcleo.
 */
public interface Plataforma {
    /** Carpeta del juego de la instancia (la de mods, screenshots…). */
    File carpeta();

    /** Tamaño de la ventana en píxeles reales. */
    int anchoVentana();

    int altoVentana();

    /** Escala de la interfaz de Minecraft (1 píxel de la interfaz = escala píxeles reales). */
    int escala();

    void abrirPanel();

    void cerrarPanel();

    boolean panelAbierto();

    /** Sale del mundo (si hay uno) y entra al servidor. */
    void conectar(String direccion);

    /** Ejecuta en el hilo del juego. */
    void enJuego(Runnable tarea);

    String portapapeles();

    void copiar(String texto);

    /** Por ejemplo "Fabric 1.21.1". */
    String descripcion();

    /** El nombre de la tecla de Parabellum (para los avisos), por ejemplo "O". */
    String tecla();
}
