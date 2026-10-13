package mx.parabellum.juego.fabric;

import com.mojang.blaze3d.platform.InputConstants;
import com.mojang.blaze3d.platform.Window;
import mx.parabellum.juego.Imagen;
import mx.parabellum.juego.Juego;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.CharacterEvent;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.network.chat.Component;

/** La pantalla del panel: dibuja la imagen del núcleo y le pasa el ratón y el teclado. */
public final class PantallaPanel extends Screen {
    private final Screen anterior;

    PantallaPanel(Screen anterior) {
        super(Component.literal("Parabellum"));
        this.anterior = anterior;
    }

    private int s() {
        return Math.max(1, minecraft.getWindow().getGuiScale());
    }

    @Override
    protected void init() {
        Juego.actual().panelAbierto();
        // Desde la 26.3 (SDL3) las letras solo llegan si alguien pide entrada de texto, como hace una
        // caja de texto del juego: el panel la pide mientras está abierto.
        minecraft.textInputManager().startTextInput(this);
    }

    @Override
    public void extractRenderState(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {
        super.extractRenderState(g, mouseX, mouseY, delta);
        Juego juego = Juego.actual();
        Window w = minecraft.getWindow();
        int s = s();
        Imagen img = juego.dibujarPanel(w.getWidth(), w.getHeight(), s);
        Texturas.dibujar(g, "panel", img, juego.panelX() / s, (juego.panelY() + juego.panelDesliz(s)) / s, img.ancho / s, img.alto / s, juego.panelAlfa());
    }

    @Override
    public void mouseMoved(double x, double y) {
        Juego.actual().raton(x * s(), y * s());
    }

    @Override
    public boolean mouseClicked(MouseButtonEvent evento, boolean doble) {
        Juego.actual().clic(evento.x() * s(), evento.y() * s(), boton(evento.button()));
        return true;
    }

    /** Desde la 26.3 los botones del ratón también son de SDL3 (izquierdo 1, derecho 3): al núcleo van
     * con los números de GLFW (izquierdo 0, derecho 1, central 2). */
    private static int boton(int b) {
        if (b == InputConstants.MOUSE_BUTTON_LEFT) return 0;
        if (b == InputConstants.MOUSE_BUTTON_RIGHT) return 1;
        if (b == InputConstants.MOUSE_BUTTON_MIDDLE) return 2;
        return b;
    }

    @Override
    public boolean mouseScrolled(double x, double y, double horizontal, double vertical) {
        return Juego.actual().rueda(x * s(), y * s(), vertical);
    }

    /** Desde la 26.3 las teclas son de SDL3: se pasan al núcleo con los números de GLFW. */
    private static int tecla(KeyEvent e) {
        if (e.isEscape()) return 256;
        if (e.isConfirmation()) return 257;
        if (e.isLeft()) return 263;
        if (e.isRight()) return 262;
        if (e.isUp()) return 265;
        if (e.isDown()) return 264;
        if (e.isPaste()) return 86;
        if (e.isSelectAll()) return 65;
        if (e.isCopy()) return 67;
        int k = e.key();
        if (k == InputConstants.KEY_BACKSPACE) return 259;
        if (k == InputConstants.KEY_DELETE) return 261;
        if (k == InputConstants.KEY_HOME) return 268;
        if (k == InputConstants.KEY_END) return 269;
        return -1;
    }

    @Override
    public boolean keyPressed(KeyEvent evento) {
        boolean ctrl = evento.hasControlDown() || evento.isPaste() || evento.isSelectAll() || evento.isCopy();
        if (Juego.actual().teclaPanel(tecla(evento), ctrl ? 2 : 0)) return true;
        if (ParabellumFabric.ABRIR.matches(evento)) {
            onClose();
            return true;
        }
        return super.keyPressed(evento);
    }

    @Override
    public boolean charTyped(CharacterEvent evento) {
        boolean usado = false;
        for (char c : Character.toChars(evento.codepoint())) usado |= Juego.actual().caracter(c);
        return usado;
    }

    @Override
    public void onClose() {
        minecraft.gui.setScreen(anterior);
    }

    @Override
    public void removed() {
        minecraft.textInputManager().stopTextInput(this);
        Juego.actual().panelCerrado();
        Texturas.liberar("panel");
    }

    @Override
    public boolean isPauseScreen() {
        return false;
    }
}
