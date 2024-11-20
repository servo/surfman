package org.mozilla.surfmanthreadsexample;

import android.content.Context;
import android.support.test.InstrumentationRegistry;
import android.support.test.runner.AndroidJUnit4;

import org.junit.Test;
import org.junit.runner.RunWith;

import static org.junit.Assert.*;

/**
 * Instrumented test, which will execute on an Android device.
 *
 * @see <a href="http://d.android.com/tools/testing">Testing documentation</a>
 */
@RunWith(AndroidJUnit4.class)
public class SurfmanInstrumentedTest {
    private static native void testContextCreation();
    private static native void testCrossDeviceSurfaceTextureBlitFramebuffer();
    private static native void testCrossThreadSurfaceTextureBlitFramebuffer();
    private static native void testDeviceAccessors();
    private static native void testDeviceCreation();
    private static native void testGenericSurfaceCreation();
    private static native void testGL();
    private static native void testNewlyCreatedContextsAreCurrent();
    private static native void testSurfaceTextureBlitFramebuffer();
    private static native void testSurfaceTextureRightSideUp();

    static {
        System.loadLibrary("surfman_android_threads");
    }

    @Test
    public void useAppContext() {
        // Context of the app under test.
        Context appContext = InstrumentationRegistry.getTargetContext();

        assertEquals("org.mozilla.surfmanthreadsexample", appContext.getPackageName());
    }

    @Test
    public void contextCreation() {
        testContextCreation();
    }

    @Test
    public void crossDeviceSurfaceTextureBlitFramebuffer() {
        testCrossDeviceSurfaceTextureBlitFramebuffer();
    }

    @Test
    public void crossThreadSurfaceTextureBlitFramebuffer() {
        testCrossThreadSurfaceTextureBlitFramebuffer();
    }

    @Test
    public void deviceAccessors() {
        testDeviceAccessors();
    }

    @Test
    public void deviceCreation() {
        testDeviceCreation();
    }

    @Test
    public void genericSurfaceCreation() {
        testGenericSurfaceCreation();
    }

    @Test
    public void gl() {
        testGL();
    }

    @Test
    public void newlyCreatedContextsAreCurrent() {
        testNewlyCreatedContextsAreCurrent();
    }

    @Test
    public void surfaceTextureBlitFramebuffer() {
        testSurfaceTextureBlitFramebuffer();
    }

    @Test
    public void surfaceTextureRightSideUp() {
        testSurfaceTextureRightSideUp();
    }
}
