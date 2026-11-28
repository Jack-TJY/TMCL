// Isolated loader for the 1.18 probe: the official jars are signed.
package helper;
import java.net.*;
import java.nio.file.*;
public class MansionRun {
  public static void main(String[] args) throws Exception {
    var child = new URLClassLoader(new URL[] {Path.of(".").toUri().toURL()},
        MansionRun.class.getClassLoader());
    Class.forName("Mansion18", true, child)
        .getMethod("main", String[].class)
        .invoke(null, (Object) args);
  }
}
