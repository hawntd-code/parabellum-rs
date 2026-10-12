package mx.parabellum.juego.fabric.mixin;

import mx.parabellum.juego.mc.Base;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.gui.Gui;
import net.minecraft.client.gui.GuiGraphics;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Los avisos, encima de la interfaz del juego. */
@Mixin(Gui.class)
public class GuiMixin {
    @Inject(method = "render", at = @At("TAIL"))
    private void parabellum$avisos(GuiGraphics g, DeltaTracker delta, CallbackInfo ci) {
        Base.dibujarAvisos(g);
    }
}
