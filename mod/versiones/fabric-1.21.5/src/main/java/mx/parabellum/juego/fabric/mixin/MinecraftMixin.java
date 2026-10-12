package mx.parabellum.juego.fabric.mixin;

import mx.parabellum.juego.mc.Base;
import net.minecraft.client.Minecraft;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Cada tick del cliente: puente, avisos, capturas nuevas y la tecla. */
@Mixin(Minecraft.class)
public class MinecraftMixin {
    @Inject(method = "tick", at = @At("TAIL"))
    private void parabellum$tick(CallbackInfo ci) {
        Base.tick();
    }
}
