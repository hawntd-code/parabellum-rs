package mx.parabellum.juego.fabric.mixin;

import mx.parabellum.juego.mc.Base;
import net.minecraft.client.gui.screens.PauseScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** El botón de Parabellum en el menú de pausa. */
@Mixin(PauseScreen.class)
public abstract class PauseScreenMixin extends Screen {
    protected PauseScreenMixin(Component titulo) {
        super(titulo);
    }

    @Inject(method = "init", at = @At("TAIL"))
    private void parabellum$boton(CallbackInfo ci) {
        addRenderableWidget(Base.botonPausa(this));
    }
}
