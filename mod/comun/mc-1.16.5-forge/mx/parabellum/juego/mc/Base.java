package mx.parabellum.juego.mc;

import net.minecraft.client.util.InputMappings;
import net.minecraft.client.MainWindow;
import mx.parabellum.juego.Imagen;
import mx.parabellum.juego.Juego;
import mx.parabellum.juego.Plataforma;
import net.minecraft.util.SharedConstants;
import net.minecraft.client.settings.KeyBinding;
import net.minecraft.client.Minecraft;
import com.mojang.blaze3d.matrix.MatrixStack;
import net.minecraft.client.gui.screen.ConnectingScreen;
import net.minecraft.client.gui.screen.DirtMessageScreen;
import net.minecraft.client.gui.screen.MainMenuScreen;
import net.minecraft.client.multiplayer.ServerData;
import net.minecraft.util.text.StringTextComponent;
import net.minecraft.util.text.TranslationTextComponent;
import org.lwjgl.glfw.GLFW;

import java.io.File;

/**
 * Minecraft 1.16.5 en Forge: clases con los nombres de MCP (Forge 1.16 no renombra clases) y métodos con
 * los de Mojang. Java 8: sin novedades del lenguaje.: une el núcleo con el juego. Cada loader solo
 * registra la tecla y llama a {@link #tick} y {@link #dibujarAvisos}.
 */
public abstract class Base implements Plataforma {
    /** La tecla (en Controles › Multijugador). */
    public static final KeyBinding ABRIR = new KeyBinding("key.parabellum.abrir", InputMappings.Type.KEYSYM, GLFW.GLFW_KEY_O, "key.categories.multiplayer");

    private static boolean probado;

    /** "Fabric" o "Forge". */
    protected abstract String loader();

    protected static Minecraft mc() {
        return Minecraft.getInstance();
    }

    /** Cada tick del cliente. */
    public static void tick() {
        Juego juego = Juego.actual();
        if (juego == null) return;
        juego.tick();
        while (ABRIR.consumeClick()) juego.tecla();
        // -Dparabellum.prueba.abrir: abre el panel solo en el menú principal (para probar sin tocar nada).
        if (!probado && System.getProperty("parabellum.prueba.abrir") != null && mc().screen instanceof MainMenuScreen && mc().getOverlay() == null) {
            probado = true;
            juego.tecla();
        }
    }

    /** Los avisos, encima de la interfaz del juego. */
    public static void dibujarAvisos(MatrixStack g) {
        Juego juego = Juego.actual();
        if (juego == null) return;
        MainWindow w = mc().getWindow();
        int s = Math.max(1, (int) w.getGuiScale());
        Imagen img = juego.avisos(w.getWidth(), w.getHeight(), s);
        if (img == null) return;
        int margen = 8 * s;
        int x = ((w.getWidth() - img.ancho - margen) / s) * s;
        Texturas.dibujar(g, "avisos", img, x / s, margen / s, img.ancho / s, img.alto / s);
    }

    @Override
    public File carpeta() {
        return mc().gameDirectory;
    }

    @Override
    public int anchoVentana() {
        return mc().getWindow().getWidth();
    }

    @Override
    public int altoVentana() {
        return mc().getWindow().getHeight();
    }

    @Override
    public int escala() {
        return Math.max(1, (int) mc().getWindow().getGuiScale());
    }

    @Override
    public void abrirPanel() {
        mc().setScreen(new PantallaPanel(mc().screen));
    }

    @Override
    public void cerrarPanel() {
        if (mc().screen instanceof PantallaPanel) ((PantallaPanel) mc().screen).onClose();
    }

    @Override
    public boolean panelAbierto() {
        return mc().screen instanceof PantallaPanel;
    }

    @Override
    public void conectar(String direccion) {
        Minecraft mc = mc();
        mc.execute(() -> {
            // Como el botón «Desconectar» del menú de pausa.
            if (mc.level != null) {
                boolean local = mc.isLocalServer();
                mc.level.disconnect();
                if (local) mc.clearLevel(new DirtMessageScreen(new TranslationTextComponent("menu.savingLevel")));
                else mc.clearLevel();
            }
            ServerData datos = new ServerData("Parabellum", direccion, false);
            mc.setScreen(new ConnectingScreen(new MainMenuScreen(), mc, datos));
        });
    }

    @Override
    public void enJuego(Runnable tarea) {
        mc().execute(tarea);
    }

    @Override
    public String portapapeles() {
        return mc().keyboardHandler.getClipboard();
    }

    @Override
    public void copiar(String texto) {
        mc().keyboardHandler.setClipboard(texto);
    }

    @Override
    public String descripcion() {
        return loader() + " " + SharedConstants.getCurrentVersion().getName();
    }

    @Override
    public String tecla() {
        return ABRIR.getTranslatedKeyMessage().getString();
    }
}
