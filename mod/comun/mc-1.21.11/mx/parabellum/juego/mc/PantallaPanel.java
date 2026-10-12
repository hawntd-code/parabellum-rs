package mx.parabellum.juego.mc;

import com.mojang.blaze3d.platform.Window;
import mx.parabellum.juego.Imagen;
import mx.parabellum.juego.Juego;
import net.minecraft.client.gui.GuiGraphics;
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
        return Math.max(1, (int) minecraft.getWindow().getGuiScale());
    }

    @Override
    protected void init() {
        Juego.actual().panelAbierto();
    }

    @Override
    public void render(GuiGraphics g, int mouseX, int mouseY, float delta) {
        super.render(g, mouseX, mouseY, delta);
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
        Juego.actual().clic(evento.x() * s(), evento.y() * s(), evento.button());
        return true;
    }

    @Override
    public boolean mouseScrolled(double x, double y, double horizontal, double vertical) {
        return Juego.actual().rueda(x * s(), y * s(), vertical);
    }

    @Override
    public boolean keyPressed(KeyEvent evento) {
        if (Juego.actual().teclaPanel(evento.key(), evento.modifiers())) return true;
        if (Base.ABRIR.matches(evento)) {
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
        minecraft.setScreen(anterior);
    }

    @Override
    public void removed() {
        Juego.actual().panelCerrado();
        Texturas.liberar("panel");
    }

    @Override
    public boolean isPauseScreen() {
        return false;
    }
}
