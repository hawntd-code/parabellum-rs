package mx.parabellum.juego.fabric.mixin;

import mx.parabellum.juego.fabric.ParabellumFabric;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.Gui;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Los avisos, encima de la interfaz del juego. */
@Mixin(Gui.class)
public class HudMixin {
    @Inject(method = "extractRenderState", at = @At("TAIL"))
    private void parabellum$avisos(GuiGraphicsExtractor g, DeltaTracker delta, CallbackInfo ci) {
        ParabellumFabric.dibujarAvisos(g);
    }
}
